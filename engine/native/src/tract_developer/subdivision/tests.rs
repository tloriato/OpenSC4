// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Piece geometry, resource paths, parent lifetime, and dimension scratch retention.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Size(u32, u32),
    Find(u32, u32),
    Unlink(u32, u32, u32, u32),
    Allocate(u32),
    Add(u32),
    Release(u32),
    Demolish(u32),
    Delete(u32),
    Create([u32; 5]),
    Pick(u32),
    Profile(u32),
    Claim(u32),
    Start(u32, u32, u32, u32),
    Free(u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    candidate: u32,
    found: u32,
    unlinked: u32,
    created: u32,
    create_success: VecDeque<bool>,
    selections: VecDeque<u32>,
    claims: VecDeque<u32>,
    profile: u32,
    null_parent: bool,
    mutation: Option<(u32, u32)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn size(this: u32, width: u32, depth: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let original_argument = unsafe { word(depth, 0) };
        assert_eq!(original_argument, s.candidate);
        s.calls.push(Call::Size(this, original_argument));
        unsafe {
            write_unaligned(width as *mut u8, 0xA5);
            write_unaligned(depth as *mut u8, 0x5A);
            if s.null_parent {
                store(s.candidate, 4, 0);
            }
        }
    });
    0x1111_2222_3333_4444
}
extern "thiscall" fn find(this: u32, output: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(key, s.candidate + 4);
        s.calls.push(Call::Find(this, unsafe { word(key, 0) }));
        unsafe { store(output, 0, s.found) };
    });
    0x2222_3333_4444_5555
}
extern "C" fn unlink(node: u32, root: u32, first: u32, last: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Unlink(node, root, first, last));
        0x3333_4444_0000_0000 | u64::from(s.unlinked)
    })
}
extern "C" fn allocate(bytes: u32) -> u64 {
    assert_eq!(bytes, 12);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        let mut block = vec![0xCCCC_CCCCu32; 3].into_boxed_slice();
        let p = block.as_mut_ptr() as u32;
        s.allocations.push(block);
        0x4444_5555_0000_0000 | u64::from(p)
    })
}
extern "thiscall" fn add(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Add(this)));
    0x5555_6666_7777_8888
}
extern "thiscall" fn release(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this)));
    0x6666_7777_8888_9999
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
        unsafe {
            let header = word(list, 0);
            let node = word(header, 0);
            assert_ne!(header, node);
            assert_eq!([word(node, 0), word(node, 4), word(header, 4)], [header, header, node]);
            s.calls.push(Call::Demolish(word(node, 8)));
        }
    });
    0x7777_8888_9999_AAAA
}
extern "thiscall" fn delete(_this: u32, lot: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Delete(lot)));
    0x8888_9999_AAAA_BBBB
}
extern "thiscall" fn create(
    _this: u32,
    x: u32,
    z: u32,
    w: u32,
    d: u32,
    facing: u32,
    output: u32,
    a: u32,
    b: u32,
    c: u32,
) -> u64 {
    assert_eq!([a, b, c], [0; 3]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Create([x, z, w, d, facing]));
        assert_eq!(unsafe { word(output, 0) }, 0);
        let success = s.create_success.pop_front().unwrap_or(true);
        unsafe {
            store(output, 0, if success { s.created } else { 0 });
            if let Some((this, count)) = s.mutation.take() {
                // Anchor and size are already cached, while facing and piece count are read again.
                store(s.candidate, 8, 0x1234_5678);
                store(s.candidate, 0xC, 0x8765_4321);
                store(s.candidate, 0x10, 3);
                write_unaligned((s.candidate + 0x6C) as *mut u8, count as u8);
                store(this, 0xF0, 0x5555_0000);
                store(this, 0xF4, 0x6666_0000);
            }
        }
    });
    // Only the output pointer determines whether the created lot is used.
    0x9999_AAAA_0000_0000
}
extern "thiscall" fn pick(_this: u32, candidate: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Pick(candidate));
        0xAAAA_BBBB_0000_0000 | u64::from(s.selections.pop_front().unwrap_or(27))
    })
}
extern "thiscall" fn profile(_this: u32, building: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Profile(building));
        0xBBBB_CCCC_0000_0000 | u64::from(s.profile)
    })
}
extern "thiscall" fn claim(_this: u32, capacity: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Claim(capacity));
        0xCCCC_DDDD_0000_0000 | u64::from(s.claims.pop_front().unwrap_or(0x1234_5601))
    })
}
extern "thiscall" fn start(_this: u32, lot: u32, cfg: u32, building: u32, flag: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Start(lot, cfg, building, flag)));
    0xDDDD_EEEE_0000_0000
}
extern "C" fn free(pointer: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(pointer)));
    0xEEEE_FFFF_1234_5678
}

#[unsafe(naked)]
unsafe extern "C" fn dimensions_at_entry() {
    core::arch::naked_asm!(
        "mov dword ptr [esp - 0x1c], 88", "mov dword ptr [esp - 0x18], 77",
        "jmp {entry}", entry = sym fc4p_0070a9b0,
    );
}

struct Fixture {
    object: Object,
    candidate: Object,
    tables: Vec<Box<[u32]>>,
    objects: Vec<Box<[u32]>>,
    profile: Box<[u32; 3]>,
    global: Box<u32>,
    fakes: Vec<Fake>,
}
impl Fixture {
    fn new() -> Self {
        let mut f = Self {
            object: Object::new(0x188),
            candidate: Object::new(0xA4),
            tables: Vec::new(),
            objects: Vec::new(),
            profile: Box::new([0, 0, 0xFFFF_0002]),
            global: Box::new(0),
            fakes: Vec::new(),
        };
        let parent = f.service(&[
            (0x58, size as *const () as u32),
            (4, add as *const () as u32),
            (8, release as *const () as u32),
        ]);
        let created = f.service(&[(8, release as *const () as u32)]);
        let manager = f.service(&[(0x58, create as *const () as u32), (0x60, delete as *const () as u32)]);
        let bds = f.service(&[(0x9C, profile as *const () as u32), (0xC8, claim as *const () as u32)]);
        let starter = f.service(&[(0x18, start as *const () as u32)]);
        let demolisher = f.service(&[(0x1C, demolish as *const () as u32)]);
        *f.global = demolisher;
        let p = f.object.ptr() as u32;
        let c = f.candidate.ptr() as u32;
        unsafe {
            store(p, 0x1C, bds);
            store(p, 0x28, starter);
            store(p, 0x2C, manager);
            store(p, 0x10C, 0x2222_0000);
            store(p, 0x110, 0);
            store(p, 0xF0, 0x3333_0000);
            store(p, 0xF4, 0x4444_0000);
            store(p, 0xBC, 100);
            store(p, 0xC0, 0);
            store(p, 0xC4, u32::MAX);
            store(c, 4, parent);
            store(c, 8, 10);
            store(c, 0xC, 20);
            write_unaligned((c + 0x14) as *mut u8, 2);
            write_unaligned((c + 0x15) as *mut u8, 3);
            write_unaligned((c + 0x6C) as *mut u8, 3);
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                candidate: c,
                found: 0x2222_0000,
                created,
                profile: f.profile.as_mut_ptr() as u32,
                ..State::default()
            }
        });
        f.fakes = [
            (0x0049_7520, find as *const () as usize),
            (0x0040_DAF0, unlink as *const () as usize),
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x0070_9BA0, pick as *const () as usize),
            (0x0070_A9B0, dimensions_at_entry as *const () as usize),
        ]
        .into_iter()
        .map(|(a, p)| fake(a, p))
        .collect();
        f.fakes
            .push(fake_data(0x00B4_3D00, (&mut *f.global as *mut u32).cast()));
        f
    }
    fn service(&mut self, slots: &[(usize, u32)]) -> u32 {
        let mut table = vec![0; 64].into_boxed_slice();
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        let table_ptr = table.as_ptr() as u32;
        self.tables.push(table);
        let mut object = vec![table_ptr].into_boxed_slice();
        let p = object.as_mut_ptr() as u32;
        self.objects.push(object);
        p
    }
    fn run(&mut self) -> (u32, u32, u32) {
        run(
            0x0070_A9B0,
            Conv::Thiscall,
            self.object.ptr() as u32,
            &[self.candidate.ptr() as u32],
        )
    }
}

fn creation_calls() -> Vec<[u32; 5]> {
    STATE.with(|s| {
        s.borrow()
            .calls
            .iter()
            .filter_map(|c| if let Call::Create(v) = c { Some(*v) } else { None })
            .collect()
    })
}

#[test]
fn every_facing_and_invalid_facing_scratch_produce_the_original_piece_row() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for facing in [0, 1, 2, 3, 4, 0x8000_0000, u32::MAX] {
        let mut f = Fixture::new();
        let p = f.object.ptr() as u32;
        let c = f.candidate.ptr() as u32;
        unsafe { store(c, 0x10, facing) };
        let candidate_before = snapshot(c as *mut u8, 0xA4);
        let result = f.run();
        assert_eq!((result.0, result.2), (0x1234_5678, 0xEEEE_FFFF));
        assert_eq!(snapshot(c as *mut u8, 0xA4), candidate_before);
        let wanted = match facing {
            0 => vec![[9, 20, 2, 3, 0], [7, 20, 2, 3, 0], [5, 20, 2, 3, 0]],
            1 => vec![[8, 19, 3, 2, 1], [8, 17, 3, 2, 1], [8, 15, 3, 2, 1]],
            2 => vec![[10, 18, 2, 3, 2], [12, 18, 2, 3, 2], [14, 18, 2, 3, 2]],
            3 => vec![[10, 20, 3, 2, 3], [10, 22, 3, 2, 3], [10, 24, 3, 2, 3]],
            _ => vec![[10, 20, 77, 88, facing]; 3],
        };
        assert_eq!(creation_calls(), wanted);
        assert_eq!(unsafe { [word(p, 0xC0), word(p, 0xC4)] }, [6, 6]);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Pick(..))).count(), 2);
            assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Start(..))).count(), 3);
            let header = s.allocations[0].as_ptr() as u32;
            let node = s.allocations[1].as_ptr() as u32;
            assert_eq!(
                &s.calls[s.calls.len() - 3..],
                [
                    Call::Release(unsafe { word(c, 4) }),
                    Call::Free(node),
                    Call::Free(header)
                ]
            );
            unsafe {
                assert_eq!([word(header, 0), word(header, 4)], [header; 2]);
            }
        });
    }
}

#[test]
fn failed_creation_selection_and_claim_have_distinct_call_sequences() {
    let mut f = Fixture::new();
    let p = f.object.ptr() as u32;
    let c = f.candidate.ptr() as u32;
    unsafe {
        write_unaligned((c + 0x6C) as *mut u8, 5);
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.create_success = [false, true, true, true, true].into();
        s.selections = [0, 27, 27, 27].into();
        s.claims = [0x1234_5600, 0x1234_5601, 0x1234_5601].into();
    });
    f.run();
    assert_eq!(unsafe { [word(p, 0xC0), word(p, 0xC4)] }, [4, 4]);
    STATE.with(|s| {
        let s = s.borrow();
        let steps: Vec<_> = s
            .calls
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    Call::Create(..) | Call::Pick(..) | Call::Profile(..) | Call::Claim(..) | Call::Start(..)
                )
            })
            .collect();
        let created = s.created;
        let mut wanted = vec![Call::Create([9, 20, 2, 3, 0])];
        for (i, x) in [7, 5, 3, 1].into_iter().enumerate() {
            wanted.extend([
                Call::Create([x, 20, 2, 3, 0]),
                Call::Pick(c),
                Call::Profile(0x4444_0000),
            ]);
            if i > 0 {
                wanted.push(Call::Claim(2));
            }
            if i > 1 {
                wanted.push(Call::Start(created, 0x3333_0000, 0x4444_0000, 1));
            }
        }
        assert_eq!(steps, wanted.iter().collect::<Vec<_>>());
    });
}

#[test]
fn zero_target_becomes_one_and_later_pieces_are_created_without_construction() {
    let mut f = Fixture::new();
    let p = f.object.ptr() as u32;
    unsafe { store(p, 0xBC, 0) };
    f.run();
    assert_eq!(creation_calls().len(), 3);
    assert_eq!(unsafe { [word(p, 0xC0), word(p, 0xC4)] }, [2, 2]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Profile(..))).count(), 1);
        assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Pick(..))).count(), 0);
    });
}

#[test]
fn empty_piece_count_still_demolishes_deletes_and_resets_the_subdivision_total() {
    for cached in [false, true] {
        for unlinked in [0, 0x7777_0000] {
            let mut f = Fixture::new();
            let p = f.object.ptr() as u32;
            let c = f.candidate.ptr() as u32;
            let parent = unsafe { word(c, 4) };
            unsafe {
                write_unaligned((c + 0x6C) as *mut u8, 0);
                store(c, 0x24, 0x8000_0000);
            }
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.found = if cached { 0x5555_0000 } else { 0x2222_0000 };
                s.unlinked = unlinked;
            });
            f.run();
            assert_eq!(unsafe { word(p, 0xC4) }, 0);
            assert_eq!(unsafe { word(p, 0xC0) }, 0x8000_0000);
            assert_eq!(unsafe { word(p, 0x110) }, if cached { u32::MAX } else { 0 });
            STATE.with(|s| {
                let s = s.borrow();
                let header = s.allocations[0].as_ptr() as u32;
                let node = s.allocations[1].as_ptr() as u32;
                let mut expected = vec![Call::Size(parent, c), Call::Find(p + 0x10C, parent)];
                if cached {
                    expected.push(Call::Unlink(0x5555_0000, 0x2222_0004, 0x2222_0008, 0x2222_000C));
                    if unlinked != 0 {
                        expected.push(Call::Free(unlinked));
                    }
                }
                expected.extend([
                    Call::Allocate(12),
                    Call::Add(parent),
                    Call::Allocate(12),
                    Call::Add(parent),
                    Call::Release(parent),
                    Call::Demolish(parent),
                    Call::Delete(parent),
                    Call::Release(parent),
                    Call::Free(node),
                    Call::Free(header),
                ]);
                assert_eq!(s.calls, expected);
            });
        }
    }
}

#[test]
fn callbacks_keep_cached_geometry_but_change_facing_count_and_building_selection() {
    let mut f = Fixture::new();
    let p = f.object.ptr() as u32;
    STATE.with(|s| s.borrow_mut().mutation = Some((p, 2)));
    f.run();
    assert_eq!(creation_calls(), [[9, 20, 2, 3, 0], [7, 20, 2, 3, 3]]);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.calls.iter().filter(|c| **c == Call::Profile(0x6666_0000)).count(), 2);
        assert_eq!(
            s.calls
                .iter()
                .filter(|c| **c == Call::Start(s.created, 0x5555_0000, 0x6666_0000, 1))
                .count(),
            2
        );
    });
}

#[test]
fn null_parent_after_size_query_is_still_deleted_and_null_list_payload_is_not_released() {
    let mut f = Fixture::new();
    let c = f.candidate.ptr() as u32;
    unsafe {
        write_unaligned((c + 0x6C) as *mut u8, 0);
    }
    STATE.with(|s| s.borrow_mut().null_parent = true);
    f.run();
    STATE.with(|s| {
        let s = s.borrow();
        assert!(s.calls.contains(&Call::Demolish(0)));
        assert!(s.calls.contains(&Call::Delete(0)));
        assert!(!s.calls.iter().any(|c| matches!(c, Call::Add(..) | Call::Release(..))));
    });
}

#[test]
fn signed_targets_and_wrapping_geometry_and_capacity_keep_integer_widths() {
    let mut f = Fixture::new();
    let p = f.object.ptr() as u32;
    let c = f.candidate.ptr() as u32;
    unsafe {
        store(c, 8, i32::MIN as u32);
        store(c, 0xC, u32::MAX);
        store(c, 0x10, 1);
        store(c, 0x24, 0);
        store(p, 0xC0, i32::MAX as u32);
        store(p, 0xBC, 0);
        write_unaligned((c + 0x6C) as *mut u8, 2);
    }
    f.run();
    assert_eq!(
        creation_calls(),
        [[0x7FFF_FFFE, 0xFFFF_FFFE, 3, 2, 1], [0x7FFF_FFFE, 0xFFFF_FFFC, 3, 2, 1]]
    );
    assert_eq!(unsafe { [word(p, 0xC0), word(p, 0xC4)] }, [i32::MAX as u32, 0]);
    STATE.with(|s| {
        assert!(!s
            .borrow()
            .calls
            .iter()
            .any(|c| matches!(c, Call::Profile(..) | Call::Claim(..))))
    });
}
