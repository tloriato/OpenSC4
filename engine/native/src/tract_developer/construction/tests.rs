// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Construction outcomes from the core growth cases, exercised on the original object layout.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Pick(u32, u32),
    Profile(u32, u32),
    Claim(u32),
    ReleaseResources(u32),
    Subdivide(u32, u32),
    Aggregate(u32, u32),
    Add(u32),
    Release(u32),
    Find(u32, u32),
    Erase(u32, u32),
    Allocate(u32),
    Append(u32),
    Demolish(u32),
    Clear,
    Free(u32),
    Start(u32, u32, u32, u32, u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    pick_result: u32,
    claim_result: u32,
    profile: u32,
    cache_result: u32,
    allocated: Vec<Box<[u32]>>,
    header: u32,
    lot: u32,
    aggregation_succeeds: bool,
    mutate_start: bool,
    erase_clears_lot: bool,
    clear_nulls_header: bool,
    this: u32,
    references: i32,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn pick(this: u32, candidate: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Pick(this, candidate));
        0x1111_2222_0000_0000 | u64::from(s.pick_result)
    })
}
extern "thiscall" fn profile(this: u32, building: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Profile(this, building));
        0x2222_3333_0000_0000 | u64::from(s.profile)
    })
}
extern "thiscall" fn claim(_this: u32, capacity: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Claim(capacity));
        0x3333_4444_0000_0000 | u64::from(s.claim_result)
    })
}
extern "thiscall" fn release_resources(_this: u32, capacity: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::ReleaseResources(capacity)));
    0x4444_5555_AAAA_BBBB
}
extern "thiscall" fn subdivide(this: u32, candidate: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Subdivide(this, candidate)));
    unsafe { store(this, 0xC0, 0x1234_5678) };
    0x5555_6666_BBBB_CCCC
}
extern "thiscall" fn aggregate(this: u32, candidate: u32, output: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Aggregate(this, candidate));
        unsafe {
            store(this, 0xC0, word(this, 0xC0).wrapping_sub(word(candidate, 0x24)));
            store(output, 0, if s.aggregation_succeeds { s.lot } else { 0 });
        }
        if s.aggregation_succeeds {
            s.references += 1;
        }
    });
    // Build uses the output lot even when the helper's return value is zero.
    0x6666_7777_0000_0000
}
extern "thiscall" fn add(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Add(this));
        s.references += 1;
    });
    0x7777_8888_CCCC_DDDD
}
extern "thiscall" fn release(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Release(this));
        s.references -= 1;
    });
    0x8888_9999_DDDD_EEEE
}
extern "thiscall" fn find(this: u32, output: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let lot = unsafe { word(key, 0) };
        s.calls.push(Call::Find(this, lot));
        unsafe { store(output, 0, s.cache_result) };
        // The key points at Build's local lot, not the candidate's lot field.
        if s.erase_clears_lot {
            unsafe { store(key, 0, 0) };
        }
    });
    0x9999_AAAA_EEEE_FFFF
}
extern "thiscall" fn erase(this: u32, iterator: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Erase(this, iterator)));
    0xAAAA_BBBB_FFFF_0000
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        let mut mem = vec![0xCCCC_CCCCu32; bytes as usize / 4].into_boxed_slice();
        let p = mem.as_mut_ptr() as u32;
        s.allocated.push(mem);
        s.header = p;
        0xBBBB_CCCC_0000_0000 | u64::from(p)
    })
}
extern "thiscall" fn append(this: u32, value: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let lot = unsafe { word(value, 0) };
        s.calls.push(Call::Append(lot));
        let header = unsafe { word(this, 0) };
        unsafe {
            assert_eq!([word(header, 0), word(header, 4)], [header; 2]);
            store(header, 8, lot);
        }
        if lot != 0 {
            s.references += 1;
        }
    });
    0xCCCC_DDDD_1111_2222
}
extern "thiscall" fn demolish(
    _this: u32,
    flag: u32,
    list: u32,
    a: u32,
    mask: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
) -> u64 {
    assert_eq!([flag, a, mask, b, c, d, e], [1, 0, 0x800, 0, 0, 0, 0]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let lot = unsafe { word(word(list, 0), 8) };
        s.calls.push(Call::Demolish(lot));
        // The occupied capacity is subtracted after this callback.
        unsafe { store(s.this, 0xC0, i32::MIN as u32) };
    });
    0xDDDD_EEEE_2222_3333
}
extern "thiscall" fn clear(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Clear);
        if unsafe { word(word(this, 0), 8) } != 0 {
            s.references -= 1;
        }
        if s.clear_nulls_header {
            unsafe { store(this, 0, 0) };
        }
    });
    0xEEEE_FFFF_3333_4444
}
extern "C" fn free(pointer: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(pointer)));
    0xFFFF_0000_4444_5555
}
extern "thiscall" fn start(this: u32, lot: u32, cfg: u32, building: u32, flag: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Start(this, lot, cfg, building, flag));
        if s.mutate_start {
            unsafe { store(s.this, 0xC0, i32::MAX as u32) };
        }
    });
    // A rejected construction still counts its capacity.
    0x1234_5678_0000_0000
}

struct Fixture {
    object: Object,
    candidate: Object,
    _service_table: Box<[u32; 52]>,
    _lot_table: Box<[u32; 3]>,
    _start_table: Box<[u32; 7]>,
    _demolition_table: Box<[u32; 8]>,
    service: Box<[u32; 1]>,
    lot: Box<[u32; 1]>,
    starter: Box<[u32; 1]>,
    _demolisher: Box<[u32; 1]>,
    profile: Box<[u32; 3]>,
    row: Box<[u32; 28]>,
    context: Box<[u32; 19]>,
    _global: Box<u32>,
    _fakes: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        let mut service_table = Box::new([0; 52]);
        service_table[39] = profile as *const () as u32;
        service_table[50] = claim as *const () as u32;
        service_table[51] = release_resources as *const () as u32;
        let lot_table = Box::new([0, add as *const () as u32, release as *const () as u32]);
        let mut start_table = Box::new([0; 7]);
        start_table[6] = start as *const () as u32;
        let mut demolition_table = Box::new([0; 8]);
        demolition_table[7] = demolish as *const () as u32;
        let service = Box::new([service_table.as_ptr() as u32]);
        let lot = Box::new([lot_table.as_ptr() as u32]);
        let starter = Box::new([start_table.as_ptr() as u32]);
        let demolisher = Box::new([demolition_table.as_ptr() as u32]);
        let mut global = Box::new(demolisher.as_ptr() as u32);
        let mut fakes: Vec<_> = [
            (0x0070_9BA0, pick as *const () as usize),
            (0x0070_A9B0, subdivide as *const () as usize),
            (0x0070_68E0, aggregate as *const () as usize),
            (0x0049_7520, find as *const () as usize),
            (0x0071_08D0, erase as *const () as usize),
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x007E_98C0, append as *const () as usize),
            (0x0047_3EC0, clear as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        fakes.push(fake_data(0x00B4_3D00, (&mut *global as *mut u32).cast()));
        Self {
            object: Object::new(0x188),
            candidate: Object::new(0xA4),
            _service_table: service_table,
            _lot_table: lot_table,
            _start_table: start_table,
            _demolition_table: demolition_table,
            service,
            lot,
            starter,
            _demolisher: demolisher,
            profile: Box::new([0, 0, 0xABCD_FFFF]),
            row: Box::new([0; 28]),
            context: Box::new([0; 19]),
            _global: global,
            _fakes: fakes,
        }
    }
    fn setup(&mut self, kind: u32) -> (u32, u32) {
        let p = self.object.ptr() as u32;
        let c = self.candidate.ptr() as u32;
        self.row.fill(u32::MAX);
        self.context[18] = self.row.as_mut_ptr() as u32;
        unsafe {
            store(p, 0x1C, self.service.as_mut_ptr() as u32);
            store(p, 0x28, self.starter.as_mut_ptr() as u32);
            store(p, 0x170, self.context.as_mut_ptr() as u32);
            store(p, 0x10C, 0x2222_0000);
            store(p, 0xF0, 0x3333_0000);
            store(p, 0xF4, 0x4444_0000);
            store(p, 0xC0, i32::MAX as u32);
            store(c, 0, kind);
            store(c, 4, self.lot.as_mut_ptr() as u32);
            store(c, 0x24, 19);
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                this: p,
                profile: self.profile.as_mut_ptr() as u32,
                pick_result: 27,
                claim_result: 0x1234_5601,
                cache_result: 0x2222_0000,
                lot: self.lot.as_mut_ptr() as u32,
                references: i32::from(kind == 2),
                ..State::default()
            }
        });
        (p, c)
    }
}

#[test]
fn selection_failure_returns_the_full_outcome_without_claiming_resources() {
    let mut f = Fixture::new();
    for outcome in [0, 1, 20, 0x1234_561B, u32::MAX] {
        let (p, c) = f.setup(2);
        STATE.with(|s| s.borrow_mut().pick_result = outcome);
        let before = snapshot(p as *mut u8, 0x188);
        let result = run(0x0070_B440, Conv::Thiscall, p, &[c]);
        assert_eq!((result.0, result.2), (outcome, 0x1111_2222));
        assert_eq!(snapshot(p as *mut u8, 0x188), before);
        STATE.with(|s| assert_eq!(s.borrow().calls, [Call::Pick(p, c)]));
        assert_eq!(*f.row, [u32::MAX; 28]);
    }
}

#[test]
fn failed_resource_claim_counts_only_the_no_resources_bucket() {
    let mut f = Fixture::new();
    let (p, c) = f.setup(4);
    STATE.with(|s| s.borrow_mut().claim_result = 0x1234_5600);
    let before = snapshot(p as *mut u8, 0x188);
    let result = run(0x0070_B440, Conv::Thiscall, p, &[c]);
    assert_eq!((result.0, result.2), (20, f.context.as_ptr() as u32));
    assert_eq!(snapshot(p as *mut u8, 0x188), before);
    assert_eq!(f.row[20], 0);
    for i in 0..28 {
        if i != 20 {
            assert_eq!(f.row[i], u32::MAX);
        }
    }
    STATE.with(|s| {
        assert_eq!(
            s.borrow().calls,
            [
                Call::Pick(p, c),
                Call::Profile(f.service.as_ptr() as u32, 0x4444_0000),
                Call::Claim(65535)
            ]
        )
    });
}

#[test]
fn subdivision_releases_claim_before_delegating_and_returns_success() {
    let mut f = Fixture::new();
    let (p, c) = f.setup(4);
    assert_eq!(run(0x0070_B440, Conv::Thiscall, p, &[c]).0, 27);
    assert_eq!(unsafe { word(p, 0xC0) }, 0x1234_5678);
    STATE.with(|s| {
        assert_eq!(
            s.borrow().calls,
            [
                Call::Pick(p, c),
                Call::Profile(f.service.as_ptr() as u32, 0x4444_0000),
                Call::Claim(65535),
                Call::ReleaseResources(65535),
                Call::Subdivide(p, c)
            ]
        )
    });
    assert_eq!(*f.row, [u32::MAX; 28]);
}

#[test]
fn aggregation_uses_output_lot_counts_capacity_even_when_construction_fails_and_releases_lot() {
    let mut f = Fixture::new();
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for succeeds in [false, true] {
        let (p, c) = f.setup(3);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.aggregation_succeeds = succeeds;
            s.mutate_start = true;
        });
        let result = run(0x0070_B440, Conv::Thiscall, p, &[c]);
        assert_eq!(result.0, if succeeds { 27 } else { 0 });
        assert_eq!(
            unsafe { word(p, 0xC0) },
            if succeeds {
                (i32::MAX as u32).wrapping_add(65535)
            } else {
                (i32::MAX as u32).wrapping_sub(19)
            }
        );
        let mut expected = vec![
            Call::Pick(p, c),
            Call::Profile(f.service.as_ptr() as u32, 0x4444_0000),
            Call::Claim(65535),
            Call::Aggregate(p, c),
        ];
        if succeeds {
            expected.extend([
                Call::Start(
                    f.starter.as_ptr() as u32,
                    f.lot.as_ptr() as u32,
                    0x3333_0000,
                    0x4444_0000,
                    1,
                ),
                Call::Release(f.lot.as_ptr() as u32),
            ]);
        }
        STATE.with(|s| {
            assert_eq!(s.borrow().calls, expected);
            assert_eq!(s.borrow().references, 0);
        });
        assert_eq!(f.row[0], if succeeds { u32::MAX } else { 0 });
    }
}

#[test]
fn existing_lot_is_retained_uncached_demolished_built_and_released_in_order() {
    let mut f = Fixture::new();
    for cached in [false, true] {
        for clear_nulls_header in [false, true] {
            for clear_lot in [false, true] {
                let (p, c) = f.setup(2);
                let lot = f.lot.as_ptr() as u32;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.cache_result = if cached { 0x5555_0000 } else { 0x2222_0000 };
                    s.clear_nulls_header = clear_nulls_header;
                    s.erase_clears_lot = clear_lot;
                });
                let result = run(0x0070_B440, Conv::Thiscall, p, &[c]);
                assert_eq!(result.0, if clear_lot { 0 } else { 27 });
                let mut expected = vec![
                    Call::Pick(p, c),
                    Call::Profile(f.service.as_ptr() as u32, 0x4444_0000),
                    Call::Claim(65535),
                    Call::Add(lot),
                    Call::Find(p + 0x10C, lot),
                ];
                if cached {
                    expected.push(Call::Erase(p + 0x10C, 0x5555_0000));
                }
                expected.push(Call::Allocate(12));
                if !clear_lot {
                    expected.push(Call::Add(lot));
                }
                expected.push(Call::Append(if clear_lot { 0 } else { lot }));
                if !clear_lot {
                    expected.push(Call::Release(lot));
                }
                expected.extend([Call::Demolish(if clear_lot { 0 } else { lot }), Call::Clear]);
                if !clear_nulls_header {
                    STATE.with(|s| expected.push(Call::Free(s.borrow().header)));
                }
                if !clear_lot {
                    expected.extend([
                        Call::Start(f.starter.as_ptr() as u32, lot, 0x3333_0000, 0x4444_0000, 1),
                        Call::Release(lot),
                    ]);
                }
                assert_eq!(
                    unsafe { word(p, 0xC0) },
                    (i32::MIN as u32)
                        .wrapping_sub(19)
                        .wrapping_add(if clear_lot { 0 } else { 65535 })
                );
                STATE.with(|s| assert_eq!(s.borrow().calls, expected));
                assert_eq!(unsafe { word(c, 4) }, lot);
            }
        }
    }
}

#[test]
fn other_candidate_kinds_keep_the_resource_claim_and_count_unknown_failure() {
    let mut f = Fixture::new();
    for kind in [0, 1, 5, 0x8000_0000, u32::MAX] {
        let (p, c) = f.setup(kind);
        let before = snapshot(p as *mut u8, 0x188);
        let result = run(0x0070_B440, Conv::Thiscall, p, &[c]);
        assert_eq!(result.0, 0);
        assert_eq!(result.1, 0); // wrapped unknown-failure counter
        assert_eq!(snapshot(p as *mut u8, 0x188), before);
        assert_eq!(f.row[0], 0);
        STATE.with(|s| {
            assert_eq!(
                s.borrow().calls,
                [
                    Call::Pick(p, c),
                    Call::Profile(f.service.as_ptr() as u32, 0x4444_0000),
                    Call::Claim(65535)
                ]
            )
        });
    }
}

#[test]
fn core_growth_builds_two_capacity_two_lots_past_demand_three() {
    // The Build portion of core's grow_builds_relaxed_passes_until_desired:
    // the caller stops after capacity 2 + 2 reaches its desired count of 3.
    let mut f = Fixture::new();
    let (p, c) = f.setup(3);
    f.profile[2] = 2;
    unsafe {
        store(p, 0xC0, 0);
        store(c, 0x24, 0);
    }
    STATE.with(|s| s.borrow_mut().aggregation_succeeds = true);
    assert_eq!(run(0x0070_B440, Conv::Thiscall, p, &[c]).0, 27);
    assert_eq!(unsafe { word(p, 0xC0) }, 2);
    assert_eq!(run(0x0070_B440, Conv::Thiscall, p, &[c]).0, 27);
    assert_eq!(unsafe { word(p, 0xC0) }, 4);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Start(..))).count(), 2);
        assert_eq!(s.calls.iter().filter(|c| **c == Call::Claim(2)).count(), 2);
    });
}
