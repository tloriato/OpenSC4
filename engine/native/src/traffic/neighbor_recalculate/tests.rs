// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, float_environment, invoke, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Framework,
    Query(u32, u32),
    ReleaseTemporary(usize),
    Allocate(u32),
    Demand(u32, u32),
    Output(usize),
    Cities(u32),
    Population(usize, u32),
    Position(usize),
    Property(usize, u32),
    Set(usize, u32, u32),
    Reset(usize, u32, u32),
    Exit(i32, i32),
    Get(usize, u32),
    Clear,
    Free(usize),
}
#[derive(Clone)]
struct City {
    position: (i32, i32),
    population: [i32; 3],
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    allocator: usize,
    framework: usize,
    sentinel: usize,
    first: usize,
    demands: Vec<usize>,
    cities: Vec<City>,
    holders: Vec<usize>,
    capacities: Vec<[u32; 12]>,
    positions: Vec<(Option<i32>, Option<i32>)>,
    exits: Vec<(i32, i32)>,
    reset_cache: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "C" fn framework() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().framework as *mut u8)
}
unsafe extern "thiscall" fn query(_: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) -> u32 {
    log(Call::Query(cls, iid));
    unsafe {
        out.write(STATE.with(|s| s.borrow().allocator as *mut u8));
    }
    1
}
unsafe extern "thiscall" fn release_temporary(this: *mut *mut u8) -> u32 {
    log(Call::ReleaseTemporary(unsafe { this.read() } as usize));
    unsafe {
        this.write(core::ptr::null_mut());
    }
    0
}
extern "thiscall" fn allocate(_: *mut u8, n: u32) -> *mut u8 {
    log(Call::Allocate(n));
    STATE.with(|s| s.borrow().sentinel as *mut u8)
}
extern "thiscall" fn demand(_: *mut u8, id: u32, mask: u32) -> *mut u8 {
    log(Call::Demand(id, mask));
    STATE.with(|s| s.borrow().demands[((id - 0x2010) / 0x10) as usize] as *mut u8)
}
extern "thiscall" fn output(this: *mut u8) -> *mut u8 {
    let index = STATE.with(|s| s.borrow().demands.iter().position(|&p| p == this as usize).unwrap());
    log(Call::Output(index));
    this
}
#[unsafe(naked)]
unsafe extern "C" fn output_entry() {
    core::arch::naked_asm!("call {}", "fld tbyte ptr [eax + 4]", "ret", sym output);
}
unsafe extern "thiscall" fn cities(_: *mut u8, list: *mut u32, flags: u32) -> u32 {
    log(Call::Cities(flags));
    STATE.with(|s| {
        let s = s.borrow();
        let head = unsafe { list.read() } as *mut u8;
        assert_eq!(unsafe { word(head, 0) }, head as u32);
        assert_eq!(unsafe { word(head, 4) }, head as u32);
        if s.first != 0 {
            unsafe {
                store(head, 0, s.first as u32);
            }
        }
    });
    0
}
extern "thiscall" fn population(this: *mut u8, id: u32) -> i32 {
    let index = unsafe { word(this, 4) } as usize;
    log(Call::Population(index, id));
    STATE.with(|s| s.borrow().cities[index].population[((id - 0x2010) / 0x10) as usize])
}
unsafe extern "thiscall" fn position(this: *mut u8, x: *mut i32, z: *mut i32) -> u32 {
    let index = unsafe { word(this, 4) } as usize;
    log(Call::Position(index));
    STATE.with(|s| {
        let p = s.borrow().cities[index].position;
        unsafe {
            x.write(p.0);
            z.write(p.1);
        }
    });
    0
}
unsafe extern "C" fn property(holder: *mut u8, id: u32, out: *mut i32) -> u32 {
    log(Call::Property(holder as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        let index = s.holders.iter().position(|&p| p == holder as usize).unwrap();
        let value = if id == 0x4A23_576B {
            s.positions[index].0
        } else {
            s.positions[index].1
        };
        if let Some(value) = value {
            unsafe {
                out.write(value);
            }
            0xABCD_EF01
        } else {
            0xABCD_EF00
        }
    })
}
extern "thiscall" fn set(this: *mut u8, holder: *mut u8, index: u32, value: u32) -> u32 {
    log(Call::Set(holder as usize, index, value));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim);
        let i = s.holders.iter().position(|&p| p == holder as usize).unwrap();
        s.capacities[i][index as usize] = value;
    });
    0x1234_5678
}
extern "fastcall" fn reset(holder: *mut u8, index: u32, value: u32) {
    log(Call::Reset(holder as usize, index, value));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = s.holders.iter().position(|&p| p == holder as usize).unwrap();
        s.capacities[i][index as usize] = value;
    });
}
#[unsafe(naked)]
unsafe extern "C" fn reset_entry() {
    core::arch::naked_asm!("mov edx, [esp + 4]", "push ebx", "mov ecx, edi", "call {}", "ret", sym reset);
}
extern "thiscall" fn get(this: *mut u8, holder: *mut u8, index: u32) -> u32 {
    log(Call::Get(holder as usize, index));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this as usize, s.sim);
        let i = s.holders.iter().position(|&p| p == holder as usize).unwrap();
        s.capacities[i][index as usize]
    })
}
extern "thiscall" fn exit(this: *mut u8, x: i32, z: i32) -> u32 {
    log(Call::Exit(x, z));
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(this as usize, s.sim);
        0xCAFE_0000 | u32::from(s.exits.contains(&(x, z)))
    })
}
unsafe extern "thiscall" fn clear(_: *mut u32) -> u32 {
    log(Call::Clear);
    STATE.with(|s| {
        let p = s.borrow().reset_cache;
        if p != 0 {
            unsafe {
                (p as *mut u32).write(0);
            }
        }
    });
    0
}
extern "thiscall" fn free(_: *mut u8, p: u32) -> u32 {
    log(Call::Free(p as usize));
    0
}
#[unsafe(naked)]
unsafe extern "C" fn free_entry() {
    core::arch::naked_asm!("push [esp + 4]", "call {}", "mov eax, 0x12345678", "mov ecx, 0x55667788", "mov edx, 0x99aabbcc", "ret 4", sym free);
}

struct World {
    sim: Object,
    allocator: Object,
    framework: Object,
    sentinel: Object,
    header: Object,
    demands: Vec<Object>,
    cities: Vec<Object>,
    nodes: Vec<Object>,
    pools: Vec<Object>,
    holders: Vec<Object>,
    edges: Vec<Object>,
    rows: Object,
    cells: Vec<Object>,
    cache: Box<*mut u8>,
    _manager: Object,
    _dm: Object,
    _globals: [Box<*mut u8>; 2],
    _constant: Box<f64>,
    _tables: Vec<Vec<usize>>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(count: usize) -> Self {
        let tables = vec![
            table(&[(0x4C, set as *const () as usize), (0x50, get as *const () as usize)]),
            table(&[
                (0x0C, allocate as *const () as usize),
                (0x10, free_entry as *const () as usize),
            ]),
            table(&[(0x14, query as *const () as usize)]),
            table(&[(0x54, cities as *const () as usize)]),
            table(&[(0x18, demand as *const () as usize)]),
            table(&[(0x24, output_entry as *const () as usize)]),
            table(&[
                (0x14, position as *const () as usize),
                (0x44, population as *const () as usize),
            ]),
        ];
        let mut w = Self {
            sim: object(0xD00, &tables[0]),
            allocator: object(4, &tables[1]),
            framework: object(4, &tables[2]),
            _manager: object(4, &tables[3]),
            _dm: object(4, &tables[4]),
            sentinel: Object::new(12),
            header: Object::new(0x18),
            demands: (0..3).map(|_| object(0x18, &tables[5])).collect(),
            cities: (0..count).map(|_| object(8, &tables[6])).collect(),
            nodes: (0..count).map(|_| Object::new(12)).collect(),
            pools: (0..2).map(|_| Object::new(0x18)).collect(),
            holders: (0..3).map(|_| Object::new(0x14)).collect(),
            edges: (0..4).map(|_| Object::new(32)).collect(),
            rows: Object::new(8 * 12),
            cells: (0..8).map(|_| Object::new(12)).collect(),
            cache: Box::new(core::ptr::null_mut()),
            _globals: [Box::new(core::ptr::null_mut()), Box::new(core::ptr::null_mut())],
            _constant: Box::new(4294967296.0),
            _tables: tables,
            _fakes: vec![
                fake(0x0087_93EC, framework as *const () as usize),
                fake(0x0041_1870, release_temporary as *const () as usize),
                fake(0x005F_D450, property as *const () as usize),
                fake(0x0070_FA40, exit as *const () as usize),
                fake(0x0041_1DF0, clear as *const () as usize),
                fake(0x0070_DE70, reset_entry as *const () as usize),
            ],
        };
        *w.cache = w.allocator.ptr();
        *w._globals[0] = w._manager.ptr();
        *w._globals[1] = w._dm.ptr();
        for (address, pointer) in [
            (0x00B2_07E0, (&mut *w.cache as *mut *mut u8).cast()),
            (0x00B4_3D8C, (&mut *w._globals[0] as *mut *mut u8).cast()),
            (0x00B4_3D74, (&mut *w._globals[1] as *mut *mut u8).cast()),
            (0x00A8_0AB8, (&mut *w._constant as *mut f64).cast()),
        ] {
            w._fakes.push(fake_data(address, pointer));
        }
        for i in 0..count {
            unsafe {
                store(w.cities[i].ptr(), 4, i as u32);
                store(w.nodes[i].ptr(), 8, w.cities[i].ptr() as u32);
                let next = if i + 1 == count {
                    w.sentinel.ptr()
                } else {
                    w.nodes[i + 1].ptr()
                };
                store(w.nodes[i].ptr(), 0, next as u32);
            }
        }
        let sim = unsafe { CapacitySim::from_ptr(w.sim.ptr()) };
        sim.set_pools(w.header.ptr());
        sim.set_default_pool(w.holders[2].ptr());
        sim.set_edge_length(2);
        sim.set_south_edge(4);
        sim.set_destination_rows(w.rows.ptr());
        for i in 0..4 {
            unsafe {
                store(w.sim.ptr(), 0x88 + i * 12, w.edges[i].ptr() as u32);
            }
        }
        for i in 0..8 {
            unsafe {
                store(w.rows.ptr(), i * 12, w.cells[i].ptr() as u32);
            }
        }
        for i in 0..4 {
            unsafe {
                store(w.sim.ptr(), 0x1B0 + i * 4, 0);
            }
        }
        let h = unsafe { PoolNode::from_ptr(w.header.ptr()) };
        h.set_left(w.pools[0].ptr());
        h.set_parent(w.pools[0].ptr());
        h.set_right(w.pools[1].ptr());
        for i in 0..2 {
            let p = unsafe { PoolNode::from_ptr(w.pools[i].ptr()) };
            p.set_key(i as u16 + 1);
            p.set_holder(w.holders[i].ptr());
            p.set_parent(if i == 0 { w.header.ptr() } else { w.pools[0].ptr() });
            if i == 0 {
                p.set_right(w.pools[1].ptr());
            }
        }
        for (i, value) in [800.0, 10.0, 0.0].into_iter().enumerate() {
            w.demand(i, F80::from_f64(value));
        }
        STATE.with(|s| {
            *s.borrow_mut() = State {
                sim: w.sim.ptr() as usize,
                allocator: w.allocator.ptr() as usize,
                framework: w.framework.ptr() as usize,
                sentinel: w.sentinel.ptr() as usize,
                first: if count == 0 { 0 } else { w.nodes[0].ptr() as usize },
                demands: w.demands.iter_mut().map(|o| o.ptr() as usize).collect(),
                cities: vec![
                    City {
                        position: (10, 20),
                        population: [300, 1, 0],
                    },
                    City {
                        position: (11, 20),
                        population: [100, 2, 0],
                    },
                ][..count]
                    .to_vec(),
                holders: w.holders.iter_mut().map(|o| o.ptr() as usize).collect(),
                capacities: vec![[0, 0, 0, u32::MAX, 33, 33, 33, 44, 0, 0, 0, 0]; 3],
                positions: vec![(Some(10), Some(20)), (Some(11), Some(20)), (None, None)],
                ..State::default()
            }
        });
        w
    }
    fn demand(&mut self, index: usize, value: F80) {
        unsafe {
            core::ptr::copy_nonoverlapping(value.to_bytes().as_ptr(), self.demands[index].ptr().add(4), 10);
        }
    }
    fn run(&mut self) -> Registers {
        unsafe { invoke(fc4p_00714e60 as *const () as usize, self.sim.ptr(), &[], 0, 0, false) }
    }
    fn rec(&mut self, side: usize, index: usize) -> ConnectionRecord {
        unsafe { ConnectionRecord::from_ptr(self.edges[side].ptr().add(index * 8)) }
    }
    fn cell(&mut self, x: i32, z: i32) -> u8 {
        unsafe { self.cells[(x + 1) as usize].ptr().add((z + 1) as usize).read() }
    }
    fn capacity(&self, i: usize) -> [u32; 12] {
        STATE.with(|s| s.borrow().capacities[i])
    }
    fn prefix(&mut self) -> Vec<Call> {
        let mut out = vec![Call::Allocate(12)];
        for w in 0..3 {
            out.extend([Call::Demand(0x2010 + w as u32 * 0x10, 0x10000), Call::Output(w)]);
        }
        out.push(Call::Cities(0));
        for i in 0..self.cities.len() {
            for w in 0..3 {
                out.push(Call::Population(i, 0x2010 + w * 0x10));
            }
        }
        out
    }
    fn expected_city(&mut self, city: usize, capacities: [u32; 3]) -> Vec<Call> {
        let mut out = vec![Call::Position(city)];
        for i in 0..=city {
            let h = self.holders[i].ptr() as usize;
            out.extend([Call::Property(h, 0x4A23_576B), Call::Property(h, 0x4A23_576C)]);
        }
        let h = self.holders[city].ptr() as usize;
        for (w, cap) in capacities.into_iter().enumerate() {
            out.extend([
                Call::Population(city, 0x2010 + w as u32 * 0x10),
                Call::Set(h, w as u32, cap),
                Call::Reset(h, w as u32 + 4, 0),
            ]);
        }
        out.extend([Call::Set(h, 3, u32::MAX), Call::Reset(h, 7, 0)]);
        out
    }
}

#[test]
fn regional_jobs_follow_the_population_example_and_count_existing_destination_bits_again() {
    let _fpu = GameFpu::new();
    let mut w = World::new(2);
    w.rec(0, 1).set_mask(4);
    w.rec(0, 1).set_pool(1);
    w.rec(2, 0).set_mask(4);
    w.rec(2, 0).set_pool(2);
    STATE.with(|s| s.borrow_mut().exits.push((-1, 1)));
    let env = float_environment();
    assert_eq!(w.run().ecx, 0x5566_7788);
    assert_eq!(float_environment(), env);
    assert_eq!(w.capacity(0), [600, 3, 0, u32::MAX, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(w.capacity(1), [200, 6, 0, u32::MAX, 0, 0, 0, 0, 0, 0, 0, 0]);
    assert_eq!(w.capacity(2)[7], 0);
    assert_eq!(w.cell(-1, 1), 0b1011);
    assert_eq!(w.cell(3, 0), 0);
    let mut expected = w.prefix();
    expected.extend(w.expected_city(0, [600, 3, 0]));
    expected.extend(w.expected_city(1, [200, 6, 0]));
    expected.extend([Call::Reset(w.holders[2].ptr() as usize, 7, 0), Call::Exit(-1, 1)]);
    for kind in 0..4 {
        expected.push(Call::Get(w.holders[0].ptr() as usize, kind));
    }
    expected.extend([Call::Exit(3, 0), Call::Clear, Call::Free(w.sentinel.ptr() as usize)]);
    assert_eq!(calls(), expected);
    w.run();
    calls();
    for (kind, want) in [2, 2, 0, 2].into_iter().enumerate() {
        assert_eq!(unsafe { word(w.sim.ptr(), 0x1B0 + kind * 4) }, want);
    }
}

#[test]
fn border_scan_resets_invalid_pool_keys_and_uses_all_four_sides() {
    let _fpu = GameFpu::new();
    let mut w = World::new(0);
    for side in 0..4 {
        w.rec(side, 0).set_mask(1);
        w.rec(side, 0).set_pool(if side == 0 { 42 } else { 0 });
    }
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.exits = vec![(-1, 0), (0, -1), (3, 0), (0, 5)];
        s.capacities[2][0] = 9;
    });
    unsafe {
        store(w.sim.ptr(), 0x1B0, u32::MAX);
    }
    let mut expected_bytes = unsafe { bytes(w.sim.ptr(), 0xD00) };
    expected_bytes[0x1B0..0x1B4].copy_from_slice(&3u32.to_le_bytes());
    expected_bytes[0x1BC..0x1C0].copy_from_slice(&4u32.to_le_bytes());
    w.run();
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, expected_bytes);
    assert_eq!(w.rec(0, 0).pool(), 0);
    for (x, z) in [(-1, 0), (0, -1), (3, 0), (0, 5)] {
        assert_eq!(w.cell(x, z), 0b1001);
    }
    let mut expected = w.prefix();
    let h = w.holders[2].ptr() as usize;
    expected.push(Call::Reset(h, 7, 0));
    for (x, z) in [(-1, 0), (0, -1), (3, 0), (0, 5)] {
        expected.push(Call::Exit(x, z));
        for k in 0..4 {
            expected.push(Call::Get(h, k));
        }
    }
    expected.extend([Call::Clear, Call::Free(w.sentinel.ptr() as usize)]);
    assert_eq!(calls(), expected);
}

#[test]
fn unsigned_totals_and_job_words_use_x87_correction_and_zero_division_truncates_to_zero() {
    let _fpu = GameFpu::new();
    for (pop, jobs, expected) in [
        ([i32::MAX, 0, 0], F80::from_i64(3_000_000_000), 3_000_000_000),
        ([-1, 0, 0], F80::from_i32(1), 0),
        ([0, 0, 0], F80::from_i32(100), 0),
        ([300, 1, 0], F80::from_f64(6.999_999_9), 6),
    ] {
        let mut w = World::new(1);
        w.demand(0, jobs);
        STATE.with(|s| s.borrow_mut().cities[0].population = pop);
        w.run();
        assert_eq!(w.capacity(0)[0], expected);
        calls();
    }
}

#[test]
fn missing_position_properties_skip_only_the_affected_pool_and_empty_dimensions_skip_edges() {
    let _fpu = GameFpu::new();
    let mut w = World::new(2);
    STATE.with(|s| s.borrow_mut().positions[0].0 = None);
    unsafe { CapacitySim::from_ptr(w.sim.ptr()) }.set_edge_length(-1);
    w.rec(0, 0).set_mask(1);
    w.run();
    assert_eq!(w.capacity(0)[..4], [0, 0, 0, u32::MAX]);
    assert_eq!(w.capacity(1)[..4], [200, 6, 0, u32::MAX]);
    let actual = calls();
    assert!(!actual.iter().any(|c| matches!(c, Call::Exit(..))));
    let h = w.holders[0].ptr() as usize;
    assert_eq!(
        actual.iter().filter(|c| **c == Call::Property(h, 0x4A23_576B)).count(),
        2
    );
    assert!(!actual.contains(&Call::Property(h, 0x4A23_576C)));
}

#[test]
fn empty_recalculation_balances_allocator_acquisition_and_reacquisition() {
    let _fpu = GameFpu::new();
    let mut w = World::new(0);
    *w.cache = core::ptr::null_mut();
    STATE.with(|s| s.borrow_mut().reset_cache = (&mut *w.cache as *mut *mut u8) as usize);
    w.run();
    assert_eq!(*w.cache, w.allocator.ptr());
    let allocator = w.allocator.ptr() as usize;
    let mut expected = vec![
        Call::Framework,
        Call::Query(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseTemporary(allocator),
    ];
    expected.extend(w.prefix());
    expected.extend([
        Call::Reset(w.holders[2].ptr() as usize, 7, 0),
        Call::Clear,
        Call::Framework,
        Call::Query(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseTemporary(allocator),
        Call::Free(w.sentinel.ptr() as usize),
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn population_totals_wrap_as_unsigned_and_nonfinite_jobs_truncate_to_zero() {
    let _fpu = GameFpu::new();
    let mut w = World::new(2);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        for city in &mut s.cities {
            city.population = [i32::MAX, 0, 0];
        }
    });
    w.run();
    assert_eq!(w.capacity(0)[0], 400);
    assert_eq!(w.capacity(1)[0], 400);
    calls();
    for job in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        let mut w = World::new(1);
        w.demand(0, F80::from_f32(job));
        w.run();
        assert_eq!(w.capacity(0)[0], 0);
        calls();
    }
}
