// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Diagnostic branches, repeated profile reads, and interface adjustment.

use super::*;
use crate::testing::{fake, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Enabled(u32),
    Type(u32),
    FindDeveloper(u32, u32),
    FindTract(u32, [u32; 2]),
    Clear(u32, u32),
    DeveloperEntry(u32, u32, [u32; 2], u32),
    TractEntry(u32),
    Copy(u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    enabled: u32,
    developer_result: u32,
    tract_result: u32,
    this: u32,
    mutate: bool,
    type_calls: u32,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn enabled(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Enabled(this));
        0xAABB_CCDD_0000_0000 | u64::from(s.enabled)
    })
}

extern "thiscall" fn developer_type(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Type(this));
        s.type_calls += 1;
        if s.mutate && s.type_calls == 2 {
            unsafe {
                store(s.this, 0x84, 0x8000_0000);
                store(s.this, 0x88, 0xFFFF_FFFF);
            }
        }
        0xABCD_EF00_0000_0000 | u64::from(s.type_calls + 4)
    })
}

extern "thiscall" fn find_developer(this: u32, output: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::FindDeveloper(this, unsafe { word(key, 0) }));
        unsafe { store(output, 0, s.developer_result) };
    });
    0xBCDE_F012_1111_2222
}

extern "thiscall" fn find_tract(this: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls
            .push(Call::FindTract(this, unsafe { [word(key, 0), word(key, 4)] }));
        0xCDEF_0123_0000_0000 | u64::from(s.tract_result)
    })
}

extern "thiscall" fn clear(this: u32, record: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Clear(this, record));
        if s.mutate {
            unsafe { store(this, 0x80, word(this, 0x80) + 4) };
        }
    });
    0xDEF0_1234_3333_4444
}

extern "thiscall" fn developer_entry(this: u32, key: u32, tract: u32, histogram: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::DeveloperEntry(
            this,
            unsafe { word(key, 0) },
            unsafe { [word(tract, 0), word(tract, 4)] },
            histogram,
        ));
    });
    0xEF01_2345_1234_5000
}

extern "thiscall" fn tract_entry(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::TractEntry(this)));
    0xF012_3456_1234_6000
}

extern "thiscall" fn copy(this: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Copy(this)));
    0x1020_3040_5060_7080
}

fn fakes() -> Vec<Fake> {
    [
        (0x006C_0030, find_developer as *const () as usize),
        (0x0069_5F70, find_tract as *const () as usize),
        (0x0070_AF20, clear as *const () as usize),
        (0x0070_CE60, developer_entry as *const () as usize),
        (0x0070_C540, tract_entry as *const () as usize),
        (0x0070_BD30, copy as *const () as usize),
    ]
    .into_iter()
    .map(|(a, f)| fake(a, f))
    .collect()
}

#[test]
fn disabled_uses_only_the_low_byte_and_preserves_memory_and_register_results() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut table = [0u32; 58];
    table[57] = enabled as *const () as u32;
    let mut service = [table.as_ptr() as u32];
    let service = service.as_mut_ptr() as u32;
    let mut object = Object::new(0x188);
    let p = object.ptr() as u32;
    unsafe { store(p, 0x1C, service) };
    let before = snapshot(p as *mut u8, 0x188);
    STATE.with(|s| {
        *s.borrow_mut() = State {
            enabled: 0x1234_5600,
            ..State::default()
        }
    });
    for address in [0x0070_CFA0, 0x0070_AD20] {
        let result = run(address, Conv::Thiscall, p, &[]);
        assert_eq!(result.0, 0x1234_5600);
        assert_eq!(result.2, 0xAABB_CCDD);
        assert_eq!(snapshot(p as *mut u8, 0x188), before);
    }
    STATE.with(|s| assert_eq!(s.borrow().calls, [Call::Enabled(service), Call::Enabled(service)]));
}

#[test]
fn growth_sign_selects_record_half_and_existing_records_are_cleared_first() {
    let _fakes = fakes();
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut service_table = [0u32; 58];
    service_table[57] = enabled as *const () as u32;
    let mut service = [service_table.as_ptr() as u32];
    let service = service.as_mut_ptr() as u32;
    let mut dev_table = [0u32; 7];
    dev_table[6] = developer_type as *const () as u32;
    let mut developers = [dev_table.as_ptr() as u32; 2];
    let developer = developers.as_mut_ptr() as u32;
    let mut object = Object::new(0x188);
    let p = object.ptr() as u32;
    let mut developer_node = [0u32; 7];
    developer_node[5] = 0x7777_0000;
    let node = developer_node.as_mut_ptr() as u32;
    unsafe {
        store(p, 0x1C, service);
        store(p, 0x17C, 0x8888_0000);
    }
    for built in [i32::MIN, -1, 0, 1, i32::MAX] {
        for found in [0, 1, 2] {
            unsafe {
                store(p, 0x80, developer);
                store(p, 0x84, 12);
                store(p, 0x88, 34);
                store(p, 0xC0, built as u32);
            }
            let before = snapshot(p as *mut u8, 0x188);
            STATE.with(|s| {
                *s.borrow_mut() = State {
                    enabled: 0x1234_5680,
                    this: p,
                    developer_result: if found == 0 { 0x8888_0000 } else { node },
                    tract_result: if found == 1 { 0x7777_0000 } else { 0x6666_0000 },
                    ..State::default()
                }
            });
            let result = run(0x0070_CFA0, Conv::Thiscall, p, &[]);
            assert_eq!((result.0, result.2), (0x5060_7080, 0x1020_3040));
            assert_eq!(snapshot(p as *mut u8, 0x188), before);
            let mut expected = vec![
                Call::Enabled(service),
                Call::Type(developer),
                Call::FindDeveloper(p + 0x17C, 5),
            ];
            if found != 0 {
                expected.push(Call::FindTract(node + 0x14, [12, 34]));
            }
            if found == 2 {
                expected.push(Call::Clear(p, 0x6666_0000 + if built > 0 { 0x18 } else { 0x58 }));
            }
            expected.extend([
                Call::Type(developer),
                Call::DeveloperEntry(p + 0x17C, 6, [12, 34], p + 0x13C),
                Call::TractEntry(0x1234_5000),
                Call::Copy(0x1234_6000 + if built > 0 { 0 } else { 0x40 }),
            ]);
            STATE.with(|s| assert_eq!(s.borrow().calls, expected));
        }
    }
}

#[test]
fn callbacks_can_replace_the_developer_and_tract_coordinates() {
    let _fakes = fakes();
    let mut service_table = [0u32; 58];
    service_table[57] = enabled as *const () as u32;
    let mut service = [service_table.as_ptr() as u32];
    let service = service.as_mut_ptr() as u32;
    let mut dev_table = [0u32; 7];
    dev_table[6] = developer_type as *const () as u32;
    let mut developers = [dev_table.as_ptr() as u32; 2];
    let dev = developers.as_mut_ptr() as u32;
    let mut object = Object::new(0x188);
    let p = object.ptr() as u32;
    let mut node = [0u32; 7];
    let node = node.as_mut_ptr() as u32;
    unsafe {
        store(p, 0x1C, service);
        store(p, 0x80, dev);
        store(p, 0xC0, 1);
    }
    STATE.with(|s| {
        *s.borrow_mut() = State {
            enabled: 1,
            this: p,
            mutate: true,
            developer_result: node,
            tract_result: 0x6666_0000,
            ..State::default()
        }
    });
    run(0x0070_CFA0, Conv::Thiscall, p, &[]);
    STATE.with(|s| {
        assert_eq!(
            s.borrow().calls,
            [
                Call::Enabled(service),
                Call::Type(dev),
                Call::FindDeveloper(p + 0x17C, 5),
                Call::FindTract(node + 0x14, [0, 0]),
                Call::Clear(p, 0x6666_0018),
                Call::Type(dev + 4),
                Call::DeveloperEntry(p + 0x17C, 6, [0x8000_0000, 0xFFFF_FFFF], p + 0x13C),
                Call::TractEntry(0x1234_5000),
                Call::Copy(0x1234_6000)
            ]
        )
    });
}

#[unsafe(naked)]
unsafe extern "C" fn release_registers() {
    core::arch::naked_asm!("mov eax, ecx", "xor edx, 0x12345678", "ret");
}

#[test]
fn persistence_release_adjusts_only_this_before_forwarding() {
    let _fake = fake(0x0070_48B0, release_registers as *const () as usize);
    for this in [0, 7, 8, 0x8000_0000, u32::MAX] {
        assert_eq!(
            run(0x0070_C8C0, Conv::Thiscall, this, &[]),
            (
                this.wrapping_sub(8),
                this.wrapping_sub(8),
                super::super::testing::EDX ^ 0x1234_5678
            )
        );
    }
}

mod build_history {
    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Enabled,
        Allocate(u32),
        AssignSet(u32),
        CandidateAllocate(u32),
        Construct(u32),
        Assign(u32, u32),
        GrowVector(u32),
        CopySet,
        CopyVector,
        Free(u32),
        ClearSet(u32),
    }

    #[derive(Default)]
    struct BuildState {
        events: Vec<Event>,
        allocations: Vec<Box<[u32]>>,
        temporary_header: u32,
        temporary_vector: u32,
        history_node: u32,
        clones: Vec<u32>,
        configuration_count: u32,
        fail_candidate: bool,
    }
    thread_local! { static BUILD: RefCell<BuildState> = RefCell::new(BuildState::default()); }

    fn memory(s: &mut BuildState, bytes: u32) -> u32 {
        let mut block = vec![0xCCCC_CCCCu32; bytes as usize / 4].into_boxed_slice();
        let pointer = block.as_mut_ptr() as u32;
        s.allocations.push(block);
        pointer
    }

    extern "thiscall" fn enabled(_this: u32) -> u64 {
        BUILD.with(|s| s.borrow_mut().events.push(Event::Enabled));
        0x1111_2222_3333_4401
    }

    extern "C" fn allocate(bytes: u32) -> u64 {
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::Allocate(bytes));
            let p = memory(s, bytes);
            match bytes {
                0x14 => s.temporary_header = p,
                0x3C => s.history_node = p,
                _ => unreachable!(),
            }
            0x2222_3333_0000_0000 | u64::from(p)
        })
    }

    extern "C" fn allocate_candidate(bytes: u32) -> u64 {
        assert_eq!(bytes, 0xA4);
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::CandidateAllocate(bytes));
            let p = if s.fail_candidate { 0 } else { memory(s, bytes) };
            s.clones.push(p);
            0x3333_4444_0000_0000 | u64::from(p)
        })
    }

    extern "thiscall" fn construct(this: u32) -> u64 {
        BUILD.with(|s| s.borrow_mut().events.push(Event::Construct(this)));
        0x4444_5555_0000_0000 | u64::from(this)
    }

    extern "thiscall" fn assign(this: u32, source: u32) -> u64 {
        BUILD.with(|s| s.borrow_mut().events.push(Event::Assign(this, source)));
        if this != 0 {
            unsafe { core::ptr::copy_nonoverlapping(source as *const u8, this as *mut u8, 0xA4) };
        }
        0x5555_6666_AAAA_BBBB
    }

    extern "thiscall" fn assign_set(this: u32, source: u32) -> u64 {
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::AssignSet(source));
            unsafe {
                let header = word(this, 0);
                assert_eq!(header, s.temporary_header);
                assert_eq!(
                    snapshot(header as *mut u8, 0x14),
                    [
                        &[0, 0xCC, 0xCC, 0xCC][..],
                        &0u32.to_le_bytes(),
                        &header.to_le_bytes(),
                        &header.to_le_bytes(),
                        &0xCCCC_CCCCu32.to_le_bytes(),
                    ]
                    .concat()
                );
                assert_eq!(word(this, 4), 0);
                store(this, 4, s.configuration_count);
                if s.configuration_count != 0 {
                    store(header, 4, 0x4444_0000);
                }
            }
        });
        0x6666_7777_BBBB_CCCC
    }

    extern "thiscall" fn grow_vector(this: u32, end: u32, value: u32, _allocator: u32, count: u32, unit: u32) -> u64 {
        assert_eq!((count, unit), (1, 1));
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            let p = unsafe { word(value, 0) };
            s.events.push(Event::GrowVector(p));
            unsafe {
                assert_eq!(end, word(this, 8));
                assert_eq!(end, 0);
                let buffer = memory(s, 64);
                s.temporary_vector = buffer;
                store(buffer, 0, p);
                store(this, 0, buffer);
                store(this, 4, buffer + 4);
                store(this, 8, buffer + 64);
            }
        });
        0x7777_8888_CCCC_DDDD
    }

    extern "thiscall" fn copy_set(this: u32, source: u32) -> u64 {
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::CopySet);
            unsafe {
                assert_eq!(word(source, 0), s.temporary_header);
                store(this, 0, 0x1111_0000);
                store(this, 4, word(source, 4));
            }
        });
        0x8888_9999_DDDD_EEEE
    }

    extern "thiscall" fn copy_vector(this: u32, source: u32) -> u64 {
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::CopyVector);
            unsafe {
                let begin = word(source, 0);
                let end = word(source, 4);
                assert_eq!((end - begin) / 4, s.clones.len() as u32);
                for (i, &p) in s.clones.iter().enumerate() {
                    assert_eq!(word(begin, i as u32 * 4), p);
                }
                store(this, 0, 0x2222_0000);
                store(this, 4, 0x2222_0000 + end - begin);
                store(this, 8, 0x2222_0000 + end - begin);
            }
        });
        0x9999_AAAA_EEEE_FFFF
    }

    extern "thiscall" fn clear_set(this: u32, root: u32) -> u64 {
        assert_eq!(root, 0x4444_0000);
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            assert_eq!(unsafe { word(this, 0) }, s.temporary_header);
            s.events.push(Event::ClearSet(root));
        });
        0xAAAA_BBBB_FFFF_0000
    }

    extern "C" fn free(pointer: u32) -> u64 {
        BUILD.with(|s| {
            let s = &mut *s.borrow_mut();
            s.events.push(Event::Free(pointer));
            if pointer == s.temporary_header {
                unsafe {
                    assert_eq!(word(pointer, 4), 0);
                    assert_eq!(word(pointer, 8), pointer);
                    assert_eq!(word(pointer, 0xC), pointer);
                }
            }
        });
        0xBBBB_CCCC_1234_5678
    }

    #[test]
    fn history_copies_candidates_and_metadata_then_releases_temporary_containers() {
        let _fpu = GameFpu::new();
        let _machine = PreserveMachine::new();
        let _fakes: Vec<_> = [
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x005E_55E0, allocate_candidate as *const () as usize),
            (0x0070_7680, construct as *const () as usize),
            (0x0070_7240, assign as *const () as usize),
            (0x0070_5A10, assign_set as *const () as usize),
            (0x0070_5AD0, copy_set as *const () as usize),
            (0x004B_03E0, copy_vector as *const () as usize),
            (0x0051_CA60, grow_vector as *const () as usize),
            (0x0070_49D0, clear_set as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        let mut table = [0u32; 58];
        table[57] = enabled as *const () as u32;
        let mut service = [table.as_ptr() as u32];
        let mut object = Object::new(0x188);
        let p = object.ptr() as u32;
        let mut candidates = [[0x1111_1111u32; 41], [0x2222_2222u32; 41]];
        let mut sources = [candidates[0].as_mut_ptr() as u32, candidates[1].as_mut_ptr() as u32];
        let mut nested = [0u32, 0, 0x8765_4321];
        let mut nested2 = [0u32, nested.as_mut_ptr() as u32];
        let mut nested3 = [nested2.as_mut_ptr() as u32];
        let mut history = [0u32; 2];
        let h = history.as_mut_ptr() as u32;
        unsafe {
            store(p, 0x1C, service.as_mut_ptr() as u32);
            store(p, 0x104, nested3.as_mut_ptr() as u32);
            store(p, 0x16C, h);
            for (offset, value) in [
                (0xF0, 0x1234_5678),
                (0xF4, 0xABCD_EF01),
                (0xF8, 0x8000_0000),
                (0xFC, 0xFFFF_FFFF),
                (0x100, 0x7FFF_FFFF),
            ] {
                store(p, offset, value);
            }
        }
        for count in [0u32, 1, 2] {
            for configurations in [0, 3] {
                for failed in [false, true] {
                    unsafe {
                        store(h, 0, h);
                        store(h, 4, h);
                        store(p, 0xE4, sources.as_mut_ptr() as u32);
                        store(p, 0xE8, sources.as_mut_ptr() as u32 + count * 4);
                        write_unaligned((p + 0xC8) as *mut u8, 0xA5);
                    }
                    BUILD.with(|s| {
                        *s.borrow_mut() = BuildState {
                            configuration_count: configurations,
                            fail_candidate: failed,
                            ..BuildState::default()
                        }
                    });
                    let before = snapshot(p as *mut u8, 0x188);
                    let result = run(0x0070_AD20, Conv::Thiscall, p, &[]);
                    assert_eq!((result.0, result.2), (0x1234_5678, 0xBBBB_CCCC));
                    assert_eq!(snapshot(p as *mut u8, 0x188), before);
                    BUILD.with(|s| {
                        let s = s.borrow();
                        let node = s.history_node;
                        unsafe {
                            assert_eq!(
                                [word(h, 0), word(h, 4), word(node, 0), word(node, 4)],
                                [node, node, h, h]
                            );
                            assert_eq!(word(node, 8), 0xCCCC_CCA5);
                            assert_eq!(word(node, 0xC), 0x1111_0000);
                            assert_eq!(word(node, 0x10), configurations);
                            assert_eq!(word(node, 0x14), 0xCCCC_CCCC);
                            assert_eq!(
                                (0..6).map(|i| word(node, 0x24 + i * 4)).collect::<Vec<_>>(),
                                [
                                    0x1234_5678,
                                    0xABCD_EF01,
                                    0x8000_0000,
                                    u32::MAX,
                                    0x7FFF_FFFF,
                                    0x8765_4321
                                ]
                            );
                        }
                        let mut expected = vec![Event::Enabled, Event::Allocate(0x14), Event::AssignSet(p + 0xD8)];
                        for (i, &clone) in s.clones.iter().enumerate() {
                            expected.push(Event::CandidateAllocate(0xA4));
                            if !failed {
                                expected.push(Event::Construct(clone));
                                assert_eq!(snapshot(clone as *mut u8, 0xA4), snapshot(sources[i] as *mut u8, 0xA4));
                            }
                            expected.push(Event::Assign(clone, sources[i]));
                            if i == 0 {
                                expected.push(Event::GrowVector(clone));
                            }
                        }
                        expected.extend([Event::Allocate(0x3C), Event::CopySet, Event::CopyVector]);
                        if count != 0 {
                            expected.push(Event::Free(s.temporary_vector));
                        }
                        if configurations != 0 {
                            expected.push(Event::ClearSet(0x4444_0000));
                        }
                        expected.push(Event::Free(s.temporary_header));
                        assert_eq!(s.events, expected);
                    });
                }
            }
        }
    }
}
