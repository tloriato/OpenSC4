// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Query and totals cases adapted from the civic system and budget tests.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Properties(usize),
    Read(usize, u32, usize),
    Budget(usize, usize, u32),
    Allocate(u32),
    Free(usize),
    Member(usize, u32),
    Query(usize, usize, bool),
    Totals(usize, usize, usize, u32, usize),
}
#[derive(Default)]
struct State {
    reads: usize,
    fail: Option<usize>,
    capacity: u32,
    effective: u32,
    served: u32,
    budget_calls: usize,
    has: [bool; 2],
    allocation: *mut u8,
    query_calls: usize,
    query_ptr: usize,
    null_buffer: bool,
}
thread_local! {
    static CALLS:RefCell<Vec<Call>>=const{RefCell::new(Vec::new())};
    static STATE:RefCell<State>=RefCell::new(State::default());
}
fn log(c: Call) {
    CALLS.with(|v| v.borrow_mut().push(c));
}
fn take() -> Vec<Call> {
    CALLS.with(|v| core::mem::take(&mut *v.borrow_mut()))
}
fn reset() {
    take();
    STATE.with(|s| {
        *s.borrow_mut() = State {
            capacity: 1000,
            effective: 501,
            served: u32::MAX,
            has: [true; 2],
            ..State::default()
        }
    });
}
extern "thiscall" fn properties(p: *mut u8) -> *mut u8 {
    log(Call::Properties(p as usize));
    p
}
extern "C" fn read(p: *mut u8, id: u32, out: *mut u32) -> u64 {
    log(Call::Read(p as usize, id, out as usize));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.reads;
        s.reads += 1;
        if s.fail == Some(i) {
            return registers(0xAABBCC00, 0xDDEEFF00);
        }
        let value = match i {
            0 | 1 => s.capacity,
            2 => s.effective,
            3 => s.served,
            _ => 0x7F800123,
        };
        unsafe {
            *out = value;
        }
        registers(0xAABBCC01, 0xDDEEFF00)
    })
}
extern "thiscall" fn budget(p: *mut u8, props: *mut u8, purpose: u32, item: *mut MaybeUninit<u32>) -> u64 {
    log(Call::Budget(p as usize, props as usize, purpose));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.budget_calls;
        s.budget_calls += 1;
        if !s.has[i] {
            return registers(0x10203000, 0x50607080);
        }
        unsafe {
            item.add(4).write(MaybeUninit::new(0x12345678 + i as u32));
            item.add(7)
                .write(MaybeUninit::new([0.75f32.to_bits(), 0.5f32.to_bits()][i]));
        }
        registers(0x10203001, 0x50607080)
    })
}

struct World {
    sim: Object,
    occupant: Object,
    budget: Object,
    st: Box<[usize; 52]>,
    ot: Box<[usize; 7]>,
    one: Box<u32>,
    zero: Box<u32>,
    _guards: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        reset();
        let mut w = Self {
            sim: Object::new(0x400),
            occupant: Object::new(4),
            budget: Object::new(4),
            st: Box::new([0; 52]),
            ot: Box::new([0; 7]),
            one: Box::new(1f32.to_bits()),
            zero: Box::new(0),
            _guards: vec![],
        };
        w.st[0xCC / 4] = budget as *const () as usize;
        w.ot[6] = properties as *const () as usize;
        unsafe {
            w.budget.ptr().cast::<*const usize>().write(w.st.as_ptr());
            w.occupant.ptr().cast::<*const usize>().write(w.ot.as_ptr());
            let sim = QuerySim::from_ptr(w.sim.ptr());
            sim.set_budget(w.budget.ptr());
            sim.set_school_divisor(25);
            sim.set_hospital_divisor(25);
        }
        w._guards.push(fake_data(0x00B0D440, (&mut *w.one as *mut u32).cast()));
        w._guards.push(fake_data(0x00B4AF90, (&mut *w.zero as *mut u32).cast()));
        for a in [0x005FD480, 0x005FD4F0] {
            w._guards.push(fake(a, read as *const () as usize));
        }
        w
    }
}

#[test]
fn building_queries_preserve_headers_use_service_specific_division_and_return_registers() {
    for school in [true, false] {
        for has in [[true, true], [true, false], [false, true], [false, false]] {
            let mut w = World::new();
            STATE.with(|s| s.borrow_mut().has = has);
            let mut out = [0xA5u8; 0x50];
            let p = out.as_mut_ptr();
            let function = if school { fc4p_006f91c0 } else { fc4p_006f7e80 };
            let result = unsafe { function(w.sim.ptr(), w.occupant.ptr(), p) };
            let q = unsafe { Query::from_ptr(p) };
            assert_eq!(&out[..0x14], &[0xA5; 0x14]);
            assert_eq!(&out[0x3C..], &[0xA5; 0x14]);
            let required = school || has[0];
            if required {
                assert_eq!((q.capacity(), q.effective(), q.served()), (1000, 501, 0));
                assert_eq!(
                    (q.capacity_staff(), q.effective_staff()),
                    (40, if school { 21 } else { 20 })
                );
                assert_eq!(q.grade().to_bits(), 0x7F800123);
                assert_eq!(
                    (q.staff_funding(), q.staff_cost()),
                    if has[0] {
                        (0.75f32.to_bits(), 0x12345678)
                    } else {
                        (1f32.to_bits(), 0)
                    }
                );
                assert_eq!(
                    (q.vehicle_funding(), q.vehicle_cost()),
                    if has[1] {
                        (0.5f32.to_bits(), 0x12345679)
                    } else {
                        (1f32.to_bits(), 0)
                    }
                );
                let eax = if has[1] {
                    if school {
                        0.5f32.to_bits() | 1
                    } else {
                        0x12345601
                    }
                } else {
                    0x10203001
                };
                let edx = if has[1] {
                    if school {
                        0x12345679
                    } else {
                        0x50607080
                    }
                } else if school {
                    0x50607080
                } else {
                    1f32.to_bits()
                };
                assert_eq!(result, registers(eax, edx));
            } else {
                assert_eq!(result, registers(0x10203000, 0x50607080));
                assert_eq!(q.served(), u32::MAX);
                assert_eq!(q.grade().to_bits(), 0xA5A5A5A5);
            }
            let calls = take();
            assert_eq!(calls.len(), if required { 8 } else { 7 });
            assert_eq!(calls[0], Call::Properties(w.occupant.ptr() as usize));
            let ids = if school {
                [0x8922041B, 0x8922041B, 0xE9244B2E, 0x09220161, 0x292453C5]
            } else {
                [0x69220415, 0x69220415, 0x49244B21, 0xC922015C, 0x092453CB]
            };
            for (i, id) in ids.into_iter().enumerate() {
                let Call::Read(props, actual, address) = calls[i + 1] else {
                    panic!()
                };
                assert_eq!((props, actual), (w.occupant.ptr() as usize, id));
                if (1..=3).contains(&i) {
                    assert_eq!(address, unsafe { p.add([0, 0x14, 0x18, 0x24][i]) } as usize);
                }
            }
            assert_eq!(
                calls[6],
                Call::Budget(
                    w.budget.ptr() as usize,
                    w.occupant.ptr() as usize,
                    if school { 0xEA5654B6 } else { 0xCA565486 }
                )
            );
            if required {
                assert_eq!(
                    calls[7],
                    Call::Budget(
                        w.budget.ptr() as usize,
                        w.occupant.ptr() as usize,
                        if school { 0x4A5654BA } else { 0xEA56549E }
                    )
                );
            }
        }
    }
}
#[test]
fn missing_required_query_properties_stop_in_order_and_keep_prior_writes() {
    for school in [true, false] {
        for fail in 0..5 {
            let mut w = World::new();
            STATE.with(|s| s.borrow_mut().fail = Some(fail));
            let mut out = [0xA5u8; 0x3C];
            let p = out.as_mut_ptr();
            let result = unsafe {
                if school {
                    fc4p_006f91c0(w.sim.ptr(), w.occupant.ptr(), p)
                } else {
                    fc4p_006f7e80(w.sim.ptr(), w.occupant.ptr(), p)
                }
            };
            assert_eq!(result, registers(0xAABBCC00, 0xDDEEFF00));
            assert_eq!(take().len(), fail + 2);
            let mut expected = [0xA5u8; 0x3C];
            for (i, offset, value) in [(1, 0x14, 1000u32), (2, 0x18, 501), (3, 0x24, u32::MAX)] {
                if fail > i {
                    expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                }
            }
            assert_eq!(out, expected);
        }
    }
}
#[test]
fn school_ceiling_wraps_unsigned_numerator_and_preserves_other_served_values() {
    for (effective, served, expected) in [(500, 75, 20), (501, 0, 21), (u32::MAX, 1, 0)] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.effective = effective;
            s.served = served;
        });
        let mut out = Object::new(0x3C);
        let p = out.ptr();
        unsafe {
            fc4p_006f91c0(w.sim.ptr(), w.occupant.ptr(), p);
        }
        let q = unsafe { Query::from_ptr(p) };
        assert_eq!(q.served(), served);
        assert_eq!(q.effective_staff(), expected);
        take();
    }
}

extern "C" fn allocate(n: u32) -> *mut u8 {
    log(Call::Allocate(n));
    STATE.with(|s| s.borrow().allocation)
}
extern "C" fn free(p: *mut u8) -> u64 {
    log(Call::Free(p as usize));
    registers(0x11111111, 0x22222222)
}
#[repr(C)]
struct Building {
    table: *const usize,
    group: u32,
}
extern "thiscall" fn member(p: *mut Building, group: u32) -> u32 {
    log(Call::Member(p as usize, group));
    u32::from(unsafe { (*p).group } == group)
}
extern "thiscall" fn school_query(p: *mut u8, b: *mut u8, out: *mut u8) -> u32 {
    query(p, b, out, true)
}
extern "thiscall" fn hospital_query(p: *mut u8, b: *mut u8, out: *mut u8) -> u32 {
    query(p, b, out, false)
}
fn query(p: *mut u8, b: *mut u8, out: *mut u8, school: bool) -> u32 {
    log(Call::Query(p as usize, b as usize, school));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let index = s.query_calls;
        s.query_calls += 1;
        unsafe {
            if index == 0 {
                s.query_ptr = out as usize;
                assert_eq!(*out.cast::<u32>(), 0x00A80810);
                assert_eq!(*out.add(0x10).cast::<u32>(), 0);
                assert_eq!(*out.add(4).cast::<*mut u8>(), s.allocation);
                assert_eq!(*s.allocation, 0);
                assert_eq!(*out.add(8).cast::<*mut u8>(), s.allocation);
                assert_eq!(*out.add(12).cast::<*mut u8>(), s.allocation.add(8));
                let q = Query::from_ptr(out);
                q.set_capacity(u32::MAX);
                q.set_effective(700);
                q.set_capacity_staff(10);
                q.set_effective_staff(7);
                q.set_served(300);
                q.set_grade(0.5);
            } else {
                assert_eq!(out as usize, s.query_ptr);
            }
            if s.null_buffer {
                Query::from_ptr(out).set_string(core::ptr::null_mut());
            }
        }
    });
    0 // A failed query still contributes the fields left in its record.
}
fn nodes(buildings: &mut [Building]) -> Vec<[u32; 3]> {
    let mut nodes = vec![[0u32; 3]; buildings.len() + 1];
    let len = nodes.len();
    let p = nodes.as_mut_ptr();
    for (i, node) in nodes.iter_mut().enumerate() {
        node[0] = unsafe { p.add((i + 1) % len) } as u32;
        if i > 0 {
            node[2] = &mut buildings[i - 1] as *mut Building as u32;
        }
    }
    nodes
}
#[test]
fn system_totals_filter_groups_count_failed_queries_reuse_record_and_leave_other_fields_untouched() {
    let _fpu = GameFpu::new();
    for school in [0, 1, 0x100] {
        let mut w = World::new();
        let _a = fake(0x0090CF54, allocate as *const () as usize);
        let _f = fake(0x0090CF63, free as *const () as usize);
        let mut storage = [0xA5u8; 8];
        STATE.with(|s| s.borrow_mut().allocation = storage.as_mut_ptr());
        let mut svt = [0usize; 28];
        svt[26] = school_query as *const () as usize;
        svt[27] = hospital_query as *const () as usize;
        unsafe {
            w.sim.ptr().cast::<*const usize>().write(svt.as_ptr());
        }
        let mut bvt = [0usize; 23];
        bvt[22] = member as *const () as usize;
        let mut buildings = [
            Building {
                table: bvt.as_ptr(),
                group: 20,
            },
            Building {
                table: bvt.as_ptr(),
                group: 30,
            },
            Building {
                table: bvt.as_ptr(),
                group: 10,
            },
        ];
        let mut nodes = nodes(&mut buildings);
        let mut heads = [nodes.as_mut_ptr() as u32];
        let mut lists = [heads.as_mut_ptr() as u32, unsafe { heads.as_mut_ptr().add(1) } as u32];
        let mut group_data = [10u32, 20];
        let mut groups = [
            group_data.as_mut_ptr() as u32,
            unsafe { group_data.as_mut_ptr().add(2) } as u32,
        ];
        let mut out = [0xA5u8; 0x50];
        let result = unsafe {
            fc4p_006fa070(
                w.sim.ptr(),
                lists.as_mut_ptr().cast(),
                out.as_mut_ptr(),
                school,
                groups.as_mut_ptr().cast(),
            )
        };
        assert_eq!(result, registers(2, 0x22222222));
        let mut expected = [0xA5u8; 0x50];
        for (offset, value) in [
            (0x14, u32::MAX - 1),
            (0x18, 1400),
            (0x1C, 20),
            (0x20, 14),
            (0x24, 600),
            (0x28, 1f32.to_bits()),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(out, expected);
        assert_eq!(storage, [0, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5, 0xA5]);
        let b: Vec<_> = buildings.iter().map(|b| b as *const Building as usize).collect();
        assert_eq!(
            take(),
            [
                Call::Allocate(8),
                Call::Member(b[0], 10),
                Call::Member(b[0], 20),
                Call::Query(w.sim.ptr() as usize, b[0], school as u8 != 0),
                Call::Member(b[1], 10),
                Call::Member(b[1], 20),
                Call::Member(b[2], 10),
                Call::Query(w.sim.ptr() as usize, b[2], school as u8 != 0),
                Call::Free(storage.as_ptr() as usize)
            ]
        );
        STATE.with(|s| s.borrow_mut().query_calls = 0);
        groups[1] = groups[0];
        assert_eq!(
            unsafe {
                fc4p_006fa070(
                    w.sim.ptr(),
                    lists.as_mut_ptr().cast(),
                    out.as_mut_ptr(),
                    school,
                    groups.as_mut_ptr().cast(),
                )
            },
            registers(0, 0x22222222)
        );
        assert_eq!(take(), [Call::Allocate(8), Call::Free(storage.as_ptr() as usize)]);
        groups[1] = unsafe { group_data.as_mut_ptr().add(2) } as u32;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.query_calls = 0;
            s.null_buffer = true;
        });
        assert_eq!(
            unsafe {
                fc4p_006fa070(
                    w.sim.ptr(),
                    lists.as_mut_ptr().cast(),
                    out.as_mut_ptr(),
                    school,
                    groups.as_mut_ptr().cast(),
                )
            },
            registers(2, lists.as_ptr() as u32)
        );
        assert!(!take().iter().any(|c| matches!(c, Call::Free(_))));
    }
}
extern "thiscall" fn totals(p: *mut u8, lists: *mut u8, out: *mut u8, school: u32, groups: *mut u8) -> u64 {
    log(Call::Totals(
        p as usize,
        lists as usize,
        out as usize,
        school,
        groups as usize,
    ));
    0x123456789ABCDEF0
}
#[test]
fn service_totals_forward_to_original_shared_entry_and_preserve_both_return_words() {
    let _fake = fake(0x006FA070, totals as *const () as usize);
    let mut sim = Object::new(0x400);
    let p = sim.ptr();
    for (function, offset, school) in [
        (
            fc4p_006fa240 as unsafe extern "thiscall" fn(*mut u8, *mut u8, *mut u8) -> u64,
            0x1A8,
            1,
        ),
        (fc4p_006fa260, 0x1B4, 0),
    ] {
        assert_eq!(
            unsafe { function(p, 0x10usize as *mut u8, 0x20usize as *mut u8) },
            0x123456789ABCDEF0
        );
        assert_eq!(
            take(),
            [Call::Totals(
                p as usize,
                unsafe { p.add(offset) } as usize,
                0x10,
                school,
                0x20
            )]
        );
    }
}
