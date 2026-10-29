// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Growth cases carried over from the core tests, with calls and bytes checked on game objects.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::tract_developer::testing::{run, snapshot, PreserveMachine};
use std::cell::RefCell;
use std::collections::VecDeque;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Call {
    Kind,
    Profile,
    Grid(u32),
    Cell(u32, u32, u32),
    Demand,
    Calendar,
    Date(u32),
    Resources,
    ClearBuilds,
    Resize(u32),
    Fill(u32, u32),
    Size,
    ClearCandidates,
    ListLots(u8),
    Reoccupy,
    Configurations,
    Existing,
    Aggregation,
    Subdivision,
    Score,
    Sort(u32, u32),
    Insertion(u32),
    Tail(u32),
    Build(u32, u8, bool),
    Allocate(u32),
    RecordBuild,
    RecordGrowth,
    Dispose(u32, u32),
    Free(u32),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    this: u32,
    purpose: u32,
    current_purpose: Option<u32>,
    grids: [u32; 4],
    demand: f32,
    demand_object: u32,
    size: u32,
    passes: usize,
    configurations: u32,
    listing: [u32; 3],
    reoccupy: u32,
    reoccupied: u32,
    out_flag: u8,
    candidates: u32,
    candidate_count: u32,
    builds: VecDeque<(u32, i32)>,
    repeat_build: (u32, i32),
    allocations: Vec<Box<[u32]>>,
    score_empties: bool,
    reverse_sort: bool,
    recorded_result: Option<(u32, u8)>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
extern "thiscall" fn kind(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Kind);
        u64::from(if s.passes == 0 {
            s.purpose
        } else {
            s.current_purpose.unwrap_or(s.purpose)
        })
    })
}
extern "thiscall" fn profile(_this: u32, output: u32) -> u64 {
    log(Call::Profile);
    unsafe {
        store(output, 0, 1);
        store(output, 4, 2);
        store(output, 8, 3);
        store(output, 12, 4);
    }
    0x1111_2222_1234_5678
}
fn grid(slot: u32, index: usize) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Grid(slot));
        u64::from(s.grids[index])
    })
}
extern "thiscall" fn desirability(_this: u32) -> u64 {
    grid(0x34, 0)
}
extern "thiscall" fn capacity(_this: u32) -> u64 {
    grid(0x38, 1)
}
extern "thiscall" fn delta(_this: u32) -> u64 {
    grid(0x3C, 2)
}
extern "thiscall" fn abandoned(_this: u32) -> u64 {
    grid(0x40, 3)
}
extern "thiscall" fn demand_object(_this: u32) -> u64 {
    STATE.with(|s| u64::from(s.borrow().demand_object))
}
extern "thiscall" fn cell(this: u32, x: u32, z: u32) -> u64 {
    log(Call::Cell(this, x, z));
    0x2233_4455_0000_0000 | u64::from(unsafe { word(this, 4) })
}
extern "thiscall" fn size(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Size);
        u64::from(s.size)
    })
}
extern "thiscall" fn demand(_this: u32) -> f32 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Demand);
        s.demand
    })
}

#[unsafe(naked)]
unsafe extern "thiscall" fn unrounded_demand(_this: u32) -> f32 {
    core::arch::naked_asm!("push 0x01000001", "fild dword ptr [esp]", "add esp, 4", "ret",);
}
extern "thiscall" fn calendar(_this: u32) -> u64 {
    log(Call::Calendar);
    0x3456_789A
}
extern "thiscall" fn date(this: u32, source: u32) -> u64 {
    log(Call::Date(source));
    unsafe {
        store(this, 0, 0xA827D8);
        store(this, 4, 0xAABB_CCDD);
    }
    0x3344_5566_1234_5678
}
extern "thiscall" fn resources(_this: u32) -> u64 {
    log(Call::Resources);
    0x4455_6677_9876_2345
}
extern "thiscall" fn clear_builds(this: u32) -> u64 {
    STATE.with(|s| assert_eq!(this, s.borrow().this + 0x16C));
    log(Call::ClearBuilds);
    0x5566_7788_1234_5678
}
extern "thiscall" fn resize(this: u32, count: u32, value: u32) -> u64 {
    STATE.with(|s| assert_eq!(this, s.borrow().this + 0x170));
    assert_eq!(unsafe { [word(value, 0), word(value, 4), word(value, 8)] }, [0; 3]);
    log(Call::Resize(count));
    0x6677_8899_1234_5678
}
extern "thiscall" fn fill(this: u32, end: u32, count: u32, value: u32) -> u64 {
    log(Call::Fill(this, count));
    assert_eq!(unsafe { word(this, 4) }, end);
    for i in 0..count {
        unsafe {
            store(end, i * 4, word(value, 0));
        }
    }
    unsafe {
        store(this, 4, end + count * 4);
    }
    0x7788_99AA_1234_5678
}
extern "C" fn clear_candidates(vector: u32) -> u64 {
    STATE.with(|s| assert_eq!(vector, s.borrow().this + 0xE4));
    log(Call::ClearCandidates);
    unsafe {
        store(vector, 4, word(vector, 0));
    }
    0x8899_AABB_1234_5678
}
extern "thiscall" fn list_lots(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.passes += 1;
        s.calls
            .push(Call::ListLots(unsafe { *(this.wrapping_add(0xC8) as *const u8) }));
        unsafe {
            *(this.wrapping_add(0x108) as *mut u8) = s.out_flag;
        }
    });
    0x99AA_BBCC_1234_5678
}
extern "thiscall" fn reoccupy(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Reoccupy);
        if s.reoccupy == 27 {
            unsafe {
                store(this, 0xC0, s.reoccupied);
            }
        }
        0xAABB_CCDD_0000_0000 | u64::from(s.reoccupy)
    })
}
extern "thiscall" fn configurations(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Configurations);
        u64::from(s.configurations)
    })
}
fn list(call: Call, index: usize) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(call);
        unsafe {
            store(s.this, 0xE4, s.candidates);
            store(s.this, 0xE8, s.candidates + s.candidate_count * 4);
        }
        u64::from(s.listing[index])
    })
}
extern "thiscall" fn existing(_this: u32) -> u64 {
    list(Call::Existing, 0)
}
extern "thiscall" fn aggregation(_this: u32) -> u64 {
    list(Call::Aggregation, 1)
}
extern "thiscall" fn subdivision(_this: u32) -> u64 {
    list(Call::Subdivision, 2)
}
extern "thiscall" fn score(this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Score);
        if s.score_empties {
            unsafe {
                store(this, 0xE8, word(this, 0xE4));
            }
        }
    });
    0xBBCC_DDEE_1234_5678
}
extern "C" fn sort(first: u32, last: u32, unused: u32, depth: u32, compare: u32) -> u64 {
    assert_eq!((unused, compare), (0, 0x0070_4770));
    log(Call::Sort((last - first) / 4, depth));
    STATE.with(|s| {
        if s.borrow().reverse_sort {
            unsafe {
                std::slice::from_raw_parts_mut(first as *mut u32, ((last - first) / 4) as usize).reverse();
            }
        }
    });
    0xCCDD_EEFF_1234_5678
}
extern "C" fn insertion(first: u32, last: u32, compare: u32) -> u64 {
    assert_eq!(compare, 0x0070_4770);
    log(Call::Insertion((last - first) / 4));
    0xDDEE_FF00_1234_5678
}
extern "C" fn tail(first: u32, last: u32, unused: u32, compare: u32) -> u64 {
    assert_eq!((unused, compare), (0, 0x0070_4770));
    log(Call::Tail((last - first) / 4));
    0xEEFF_0011_1234_5678
}
fn build(this: u32, candidate: u32, farm: bool) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Build(
            candidate,
            unsafe { *(this.wrapping_add(0xC8) as *const u8) },
            farm,
        ));
        let (code, amount) = s.builds.pop_front().unwrap_or(s.repeat_build);
        unsafe {
            store(this, 0xC0, word(this, 0xC0).wrapping_add(amount as u32));
        }
        0xFF00_1122_0000_0000 | u64::from(code)
    })
}
extern "thiscall" fn ordinary_build(this: u32, candidate: u32) -> u64 {
    build(this, candidate, false)
}
extern "thiscall" fn farm(this: u32, candidate: u32) -> u64 {
    build(this, candidate, true)
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        let mut mem = vec![0xCCCC_CCCCu32; bytes as usize / 4].into_boxed_slice();
        let p = mem.as_mut_ptr() as u32;
        s.allocations.push(mem);
        0x1122_3344_0000_0000 | u64::from(p)
    })
}
extern "thiscall" fn record_build(_this: u32) -> u64 {
    log(Call::RecordBuild);
    0x2233_4455_1234_5678
}
extern "thiscall" fn record_growth(this: u32) -> u64 {
    log(Call::RecordGrowth);
    STATE.with(|s| {
        if let Some((built, flag)) = s.borrow().recorded_result {
            unsafe {
                store(this, 0xC0, built);
                *(this.wrapping_add(0x108) as *mut u8) = flag;
            }
        }
    });
    0xA1B2_C3D4_1234_5678
}
extern "thiscall" fn dispose_lots(this: u32, root: u32) -> u64 {
    log(Call::Dispose(this, root));
    0x3344_5566_1234_5678
}
extern "C" fn free(node: u32) -> u64 {
    log(Call::Free(node));
    0x4455_6677_1234_5678
}

struct Fixture {
    object: Object,
    developer: Object,
    _table: Box<[usize; 32]>,
    _grid_table: Box<[usize; 32]>,
    _service_table: Box<[usize; 64]>,
    grids: Vec<Object>,
    services: Vec<Object>,
    rows: Object,
    bins: Vec<Object>,
    trees: Vec<Object>,
    nodes: Vec<Object>,
    list_header: Object,
    list_first: Object,
    list: [u32; 1],
    candidates: [u32; 32],
    profile: [u32; 4],
    flag: [u8; 4],
    globals: Box<[u32; 2]>,
    _fakes: Vec<Fake>,
}

impl Fixture {
    fn new() -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                purpose: 1,
                demand: 3.0,
                size: 4,
                configurations: 27,
                listing: [27; 3],
                out_flag: 0xA6,
                candidate_count: 1,
                repeat_build: (27, 2),
                ..State::default()
            }
        });
        let mut table = Box::new([0usize; 32]);
        for (slot, f) in [
            (0x1C, kind as *const ()),
            (0x24, profile as *const ()),
            (0x34, desirability as *const ()),
            (0x38, capacity as *const ()),
            (0x3C, delta as *const ()),
            (0x40, abandoned as *const ()),
            (0x48, demand_object as *const ()),
        ] {
            table[slot / 4] = f as usize;
        }
        let mut grid_table = Box::new([0usize; 32]);
        grid_table[0x28 / 4] = size as *const () as usize;
        grid_table[0x30 / 4] = demand as *const () as usize;
        grid_table[0x58 / 4] = cell as *const () as usize;
        let mut service_table = Box::new([0usize; 64]);
        service_table[0x1C / 4] = calendar as *const () as usize;
        service_table[0xC0 / 4] = resources as *const () as usize;
        let mut f = Self {
            object: Object::new(0x190),
            developer: Object::new(16),
            _table: table,
            _grid_table: grid_table,
            _service_table: service_table,
            grids: (0..5).map(|_| Object::new(16)).collect(),
            services: (0..2).map(|_| Object::new(16)).collect(),
            rows: Object::new(13 * 12),
            bins: (0..13).map(|_| Object::new(32 * 4)).collect(),
            trees: (0..3).map(|_| Object::new(24)).collect(),
            nodes: Vec::new(),
            list_header: Object::new(12),
            list_first: Object::new(12),
            list: [0],
            candidates: core::array::from_fn(|i| 0x1000 + i as u32 * 4),
            profile: [0; 4],
            flag: [0xCC; 4],
            globals: Box::new([0; 2]),
            _fakes: Vec::new(),
        };
        unsafe {
            let this = f.object.ptr() as u32;
            store(f.developer.ptr() as u32, 0, f._table.as_ptr() as u32);
            for (i, g) in f.grids.iter_mut().enumerate() {
                store(g.ptr() as u32, 0, f._grid_table.as_ptr() as u32);
                store(
                    g.ptr() as u32,
                    4,
                    [0x1234_8001, 0x5678_FEDC, 0x9ABC_FFFE, 0xDEF0_789A, 0][i],
                );
            }
            for service in &mut f.services {
                store(service.ptr() as u32, 0, f._service_table.as_ptr() as u32);
            }
            let td = TractDeveloper::from_ptr(f.object.ptr());
            GrowthState::from_ptr(f.object.ptr()).set_overbuild(1.5);
            td.set_debug_context(f.rows.ptr() as u32);
            for (i, bins) in f.bins.iter_mut().enumerate() {
                let row = f.rows.ptr() as u32 + i as u32 * 12;
                store(row, 0, bins.ptr() as u32);
                store(row, 4, bins.ptr() as u32 + 112);
                store(row, 8, bins.ptr() as u32 + 128);
                for j in 0..32 {
                    store(bins.ptr() as u32, j * 4, 0xCCCC_CCCC);
                }
            }
            for (tree, offset) in f.trees.iter_mut().zip([0xCC, 0xD8, 0x10C]) {
                let p = tree.ptr() as u32;
                store(p, 8, p);
                store(p, 12, p);
                store(this, offset, p);
            }
            let header = f.list_header.ptr() as u32;
            let node = f.list_first.ptr() as u32;
            store(header, 0, node);
            store(header, 4, node);
            store(node, 0, header);
            store(node, 4, header);
            store(node, 8, 99);
            f.list[0] = header;
            f.profile = [f.developer.ptr() as u32, 0xFFFF_FFFD, 7, 0xAABB_CCFE];
            f.globals[0] = f.services[0].ptr() as u32;
            f.globals[1] = f.services[1].ptr() as u32;
            STATE.with(|s| {
                let s = &mut *s.borrow_mut();
                s.this = this;
                for (i, g) in f.grids[..4].iter_mut().enumerate() {
                    s.grids[i] = g.ptr() as u32;
                }
                s.demand_object = f.grids[4].ptr() as u32;
            });
        }
        f._fakes = [
            (0x0091_26F3, date as *const ()),
            (0x0070_75C0, clear_builds as *const ()),
            (0x0046_2B20, resize as *const ()),
            (0x0042_3320, fill as *const ()),
            (0x0070_7370, clear_candidates as *const ()),
            (0x0070_7FB0, list_lots as *const ()),
            (0x0070_A3D0, reoccupy as *const ()),
            (0x0070_9650, configurations as *const ()),
            (0x0070_8110, existing as *const ()),
            (0x0070_97E0, aggregation as *const ()),
            (0x0070_8C00, subdivision as *const ()),
            (0x0070_5750, score as *const ()),
            (0x0070_7F20, sort as *const ()),
            (0x0070_62D0, insertion as *const ()),
            (0x0070_4A90, tail as *const ()),
            (0x0070_B440, ordinary_build as *const ()),
            (0x0070_B640, farm as *const ()),
            (0x0090_CF54, allocate as *const ()),
            (0x0070_AD20, record_build as *const ()),
            (0x0070_CFA0, record_growth as *const ()),
            (0x0070_4990, dispose_lots as *const ()),
            (0x0070_49D0, dispose_lots as *const ()),
            (0x0070_4A10, dispose_lots as *const ()),
            (0x0090_CF63, free as *const ()),
        ]
        .into_iter()
        .map(|(a, p)| fake(a, p as usize))
        .collect();
        f._fakes.push(fake_data(0x00B4_3D5C, f.globals.as_mut_ptr().cast()));
        f._fakes
            .push(fake_data(0x00B4_3D68, unsafe { f.globals.as_mut_ptr().add(1).cast() }));
        f
    }

    fn execute(&mut self) -> (u32, u32, u32) {
        STATE.with(|s| s.borrow_mut().candidates = self.candidates.as_mut_ptr() as u32);
        run(
            0x0070_D130,
            Conv::Thiscall,
            self.object.ptr() as u32,
            &[
                self.profile.as_ptr() as u32,
                self.list.as_mut_ptr() as u32,
                self.flag.as_mut_ptr() as u32,
            ],
        )
    }
    fn outcomes(&mut self) -> Vec<u32> {
        let header = self.list_header.ptr() as u32;
        let mut out = Vec::new();
        let mut node = unsafe { word(header, 0) };
        while node != header {
            out.push(unsafe { word(node, 8) });
            node = unsafe { word(node, 0) };
        }
        out
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| s.borrow().calls.clone())
    }
}

#[test]
fn core_growth_builds_relaxed_passes_until_demand_and_appends_existing_outcomes() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    let result = f.execute();
    assert_eq!((result.0, result.2), (4, f.flag.as_mut_ptr() as u32));
    assert_eq!(f.flag, [0xA6, 0xCC, 0xCC, 0xCC]);
    assert_eq!(f.outcomes(), [99, 27, 27]);
    let p = f.object.ptr() as u32;
    assert_eq!(unsafe { [word(p, 0xB8), word(p, 0xBC), word(p, 0xC0)] }, [3, 4, 4]);
    let prefix = vec![
        Call::Kind,
        Call::Profile,
        Call::Grid(0x38),
        Call::Cell(f.grids[1].ptr() as u32, 0xFFFF_FFFD, 7),
        Call::Demand,
        Call::Calendar,
        Call::Date(0x3456_789A),
        Call::Grid(0x3C),
        Call::Cell(f.grids[2].ptr() as u32, 0xFFFF_FFFD, 7),
        Call::Grid(0x40),
        Call::Cell(f.grids[3].ptr() as u32, 0xFFFF_FFFD, 7),
        Call::Grid(0x34),
        Call::Cell(f.grids[0].ptr() as u32, 0xFFFF_FFFD, 7),
        Call::Resources,
        Call::ClearBuilds,
        Call::Resize(13),
    ];
    let mut expected = prefix;
    for i in 0..13 {
        expected.push(Call::Fill(f.rows.ptr() as u32 + i * 12, 0));
    }
    expected.extend([Call::Grid(0x34), Call::Size]);
    for relaxed in [0, 1] {
        expected.extend([
            Call::ClearCandidates,
            Call::ListLots(relaxed),
            Call::Reoccupy,
            Call::Configurations,
            Call::Existing,
            Call::Aggregation,
            Call::Subdivision,
            Call::Score,
            Call::Sort(1, 0),
            Call::Insertion(1),
            Call::Kind,
            Call::Build(0x1000, relaxed, false),
            Call::Allocate(12),
            Call::RecordBuild,
        ]);
        if relaxed == 0 {
            expected.push(Call::Kind);
        }
    }
    expected.push(Call::RecordGrowth);
    assert_eq!(f.calls(), expected);
    assert_eq!(
        unsafe {
            [
                word(p, 0x140),
                word(p, 0x150),
                word(p, 0x154),
                word(p, 0x158),
                word(p, 0x15C),
                word(p, 0x160),
                word(p, 0x164),
                word(p, 0x168),
            ]
        },
        [0xAABB_CCDD, 0xFFFF_FFFE, 0, 0xFEDC, 0x789A, 0xFFFF_8001, 3, 0x2345]
    );
}

#[test]
fn demand_uses_x87_truncation_and_minimum_one_including_nonfinite_inputs() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for (demand, desired, maximum) in [
        (7.9, 7, 10),
        (0.3, 1, 1),
        (-5.0, 1, 1),
        (f32::NAN, 1, 1),
        (f32::INFINITY, 1, 1),
        (f32::NEG_INFINITY, 1, 1),
        (16_777_218.0, 16_777_218, 25_165_828),
    ] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.demand = demand;
            s.reoccupy = 27;
        });
        f.execute();
        let p = f.object.ptr() as u32;
        assert_eq!(unsafe { [word(p, 0xB8), word(p, 0xBC)] }, [desired, maximum]);
    }
}

#[test]
fn demand_stays_unrounded_in_st0_until_the_game_integer_conversion() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    f._grid_table[0x30 / 4] = unrounded_demand as *const () as usize;
    STATE.with(|s| s.borrow_mut().reoccupy = 27);
    f.execute();
    let p = f.object.ptr() as u32;
    assert_eq!(unsafe { [word(p, 0xB8), word(p, 0xBC)] }, [16_777_217, 25_165_826]);
}

#[test]
fn reoccupation_returns_signed_capacity_without_listing_and_retains_release_registers() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.reoccupy = 27;
        s.reoccupied = (-15i32) as u32;
    });
    let result = f.execute();
    assert_eq!((result.0, result.2), ((-15i32) as u32, 0xA1B2_C3A6));
    assert_eq!(f.outcomes(), [99, 27]);
    assert!(!f.calls().contains(&Call::Configurations));
    assert!(f.calls().ends_with(&[
        Call::Reoccupy,
        Call::Allocate(12),
        Call::RecordBuild,
        Call::RecordGrowth
    ]));
}

#[test]
fn pass_budget_is_size_squared_plus_two_and_one_success_ends_each_pass() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for size in [0, 1, 4, 46_341, 65_535, 0x8000_0000] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.size = size;
            s.demand = 1000.0;
            s.candidate_count = 3;
            s.repeat_build = (27, 0);
        });
        f.execute();
        let square = size.wrapping_mul(size);
        let expected = if (square as i32) < 0 {
            1
        } else {
            square.wrapping_add(2) as usize
        };
        assert_eq!(STATE.with(|s| s.borrow().passes), expected);
        assert_eq!(f.outcomes(), [vec![99], vec![27; expected]].concat());
        assert_eq!(
            f.calls().iter().filter(|c| matches!(c, Call::Build(..))).count(),
            expected
        );
    }
}

#[test]
fn no_candidates_uses_signed_maximum_maps_success_to_three_and_counts_histogram() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for (reoccupy, listing, code) in [(0, [27, 4, 5], 3), (9, [3, 4, 5], 9), (0, [u32::MAX, 7, 6], 7)] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.candidate_count = 0;
            s.reoccupy = reoccupy;
            s.listing = listing;
        });
        f.execute();
        assert_eq!(f.outcomes(), [99, code, code]);
        assert_eq!(unsafe { word(f.bins[0].ptr() as u32, code * 4) }, 2);
        assert!(!f.calls().contains(&Call::Score));
    }
}

#[test]
fn configuration_failure_stops_after_relaxed_pass_without_appending_an_outcome() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().configurations = 4);
    f.execute();
    assert_eq!(f.outcomes(), [99]);
    assert_eq!(STATE.with(|s| s.borrow().passes), 2);
    assert!(!f.calls().contains(&Call::Existing));
    assert!(!f.calls().contains(&Call::RecordBuild));
}

#[test]
fn resources_failure_retries_only_on_strict_pass_and_successful_industry_stops() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.candidate_count = 3;
        s.repeat_build = (20, 0);
    });
    f.execute();
    assert_eq!(f.outcomes(), [99, 20, 20]);
    assert_eq!(STATE.with(|s| s.borrow().passes), 2);
    for purpose in [5, 6, 7, 8] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.purpose = purpose;
            s.demand = 100.0;
        });
        assert_eq!(f.execute().0, 2);
        assert_eq!(f.outcomes(), [99, 27]);
        assert_eq!(STATE.with(|s| s.borrow().passes), 1);
        assert_eq!(f.calls().contains(&Call::Subdivision), purpose != 5);
        assert!(f.calls().contains(&Call::Build(0x1000, 0, purpose == 5)));
    }
}

#[test]
fn failed_candidates_continue_in_sorted_order_but_success_ends_the_pass() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.candidate_count = 17;
        s.reverse_sort = true;
        s.builds = [(4, 0), (22, 0), (27, 10)].into();
    });
    f.execute();
    assert_eq!(f.outcomes(), [99, 4, 22, 27]);
    assert!(f.calls().contains(&Call::Sort(17, 8)));
    assert!(f.calls().contains(&Call::Insertion(16)));
    assert!(f.calls().contains(&Call::Tail(1)));
    assert_eq!(
        f.calls()
            .into_iter()
            .filter(|c| matches!(c, Call::Build(..)))
            .collect::<Vec<_>>(),
        [
            Call::Build(0x1040, 0, false),
            Call::Build(0x103C, 0, false),
            Call::Build(0x1038, 0, false)
        ]
    );
}

#[test]
fn histogram_rows_grow_shrink_and_clear_only_the_first_twenty_eight_words() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().reoccupy = 27);
    for i in 0..13 {
        let row = f.rows.ptr() as u32 + i * 12;
        unsafe {
            store(row, 4, word(row, 0) + [0, 27, 28, 29, 32][i as usize % 5] * 4);
        }
    }
    f.execute();
    for i in 0..13 {
        let row = f.rows.ptr() as u32 + i * 12;
        assert_eq!(unsafe { word(row, 4) }, unsafe { word(row, 0) } + 112);
        let p = f.bins[i as usize].ptr() as u32;
        for j in 0..32 {
            assert_eq!(unsafe { word(p, j * 4) }, if j < 28 { 0 } else { 0xCCCC_CCCC });
        }
    }
}

#[test]
fn each_nonempty_work_tree_calls_right_subtree_helper_before_freeing_left_spine() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().reoccupy = 27);
    let this = f.object.ptr() as u32;
    let mut expected = Vec::new();
    for (i, offset) in [0xCC, 0xD8, 0x10C].into_iter().enumerate() {
        let mut node = Object::new(24);
        let p = node.ptr() as u32;
        let header = f.trees[i].ptr() as u32;
        unsafe {
            store(header, 4, p);
            store(this, offset + 4, 1);
            store(p, 12, 0xCAFE0000 + i as u32);
        }
        expected.extend([Call::Dispose(this + offset, 0xCAFE0000 + i as u32), Call::Free(p)]);
        f.nodes.push(node);
    }
    f.execute();
    assert_eq!(
        f.calls()
            .into_iter()
            .filter(|c| matches!(c, Call::Dispose(..) | Call::Free(..)))
            .collect::<Vec<_>>(),
        expected
    );
    for (i, offset) in [0xCC, 0xD8, 0x10C].into_iter().enumerate() {
        let header = f.trees[i].ptr() as u32;
        assert_eq!(
            unsafe {
                [
                    word(header, 4),
                    word(header, 8),
                    word(header, 12),
                    word(this, offset + 4),
                ]
            },
            [0, header, header, 0]
        );
    }
}

#[test]
fn scoring_can_empty_vector_and_growth_writes_no_unrelated_object_bytes() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| s.borrow_mut().score_empties = true);
    let before = snapshot(f.object.ptr(), 0x190);
    f.execute();
    assert_eq!(f.outcomes(), [99]);
    assert!(!f.calls().iter().any(|c| matches!(c, Call::Sort(..) | Call::Build(..))));
    let after = snapshot(f.object.ptr(), 0x190);
    let allowed = [0x80..0xA0, 0xB4..0xC9, 0xE4..0xEC, 0xF0..0x109, 0x140..0x16C];
    for i in 0..before.len() {
        if !allowed.iter().any(|range| range.contains(&i)) {
            assert_eq!(before[i], after[i], "byte {i:X}");
        }
    }
}

#[test]
fn profile_argument_slot_is_the_stop_byte_and_other_argument_words_stay_in_place() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for (takeover, increment, expected_stop) in [(27, 0, 0), (0, 10, 1), (0, 0, 0)] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.reoccupy = takeover;
            s.repeat_build = (27, increment);
            s.candidates = f.candidates.as_mut_ptr() as u32;
        });
        let args = [
            f.profile.as_ptr() as u32,
            f.list.as_mut_ptr() as u32,
            f.flag.as_mut_ptr() as u32,
        ];
        let mut entry = Entry {
            eax: 0x1234_5678,
            ecx: f.object.ptr() as u32,
            edx: 0x9876_5432,
            return_address: 0xAA55_AA55,
            args,
        };
        unsafe {
            grow(&mut entry);
        }
        assert_eq!(entry.args, [(args[0] & !0xFF) | expected_stop, args[1], args[2]]);
        assert_eq!(entry.return_address, 0xAA55_AA55);
        assert_eq!(f.profile[3], 0xAABB_CCFE);
        assert_eq!(unsafe { word(f.object.ptr() as u32, 0x8C) }, 0xFE);
    }
}

#[test]
fn dispatch_and_stop_requery_purpose_while_subdivision_uses_the_initial_purpose() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    let mut f = Fixture::new();
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.current_purpose = Some(5);
        s.demand = 100.0;
    });
    assert_eq!(f.execute().0, 2);
    assert!(f.calls().contains(&Call::Subdivision));
    assert!(f.calls().contains(&Call::Build(0x1000, 0, true)));
    assert_eq!(STATE.with(|s| s.borrow().passes), 1);
}

#[test]
fn final_recording_can_change_both_return_capacity_and_output_byte() {
    let _fpu = GameFpu::new();
    let _machine = PreserveMachine::new();
    for takeover in [0, 27] {
        let mut f = Fixture::new();
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.reoccupy = takeover;
            s.recorded_result = Some((0xFFFF_FEFF, 0xB7));
        });
        let result = f.execute();
        assert_eq!(result.0, 0xFFFF_FEFF);
        assert_eq!(f.flag, [0xB7, 0xCC, 0xCC, 0xCC]);
        assert_eq!(
            result.2,
            if takeover == 27 {
                0xA1B2_C3B7
            } else {
                f.flag.as_mut_ptr() as u32
            }
        );
    }
}
