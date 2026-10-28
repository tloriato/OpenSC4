// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The core's automatic-proposal cases on circular game lists and original interface slots.

use super::*;
use crate::neighbors::tests::checked_call;
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Update,
    ClearOutput,
    Current,
    Exemplar,
    Holder,
    Property(u32, u32),
    Framework,
    Service,
    ReleaseLocal,
    Allocate(bool, usize),
    Adjacent,
    Can(u32, u32),
    Find(u32, u32),
    Budget(u32),
    Max(u32, u32),
    Cost(u32, u64, u32),
    Create(u32, u32, u32),
    Clear(usize),
    Free(bool, usize),
    Erase(u32),
    Publish(u32),
    Figures(u32),
    Release(u32),
    DestroyCities(usize),
}

// List entries must stay at fixed addresses while later entries are allocated.
#[allow(clippy::vec_box)]
struct State {
    calls: Vec<Call>,
    sim: usize,
    current: usize,
    exemplar: usize,
    allocators: [usize; 2],
    caches: [usize; 2],
    output: usize,
    neighbors: Vec<usize>,
    publish_neighbors: Option<Vec<usize>>,
    adjacent_count: usize,
    allocations: Vec<(bool, Box<[u32; 3]>)>,
    nodes: Vec<Box<[u32; 3]>>,
    can: [u32; 6],
    existing: [bool; 6],
    maximum: [f64; 6],
    costs: [f64; 6],
    funds: [f64; 3],
    bases: [Option<f32>; 3],
    minimum: Option<f32>,
    ratio: Option<f32>,
    created: Vec<(u32, u32, u32)>,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            sim: 0,
            current: 0,
            exemplar: 0,
            allocators: [0; 2],
            caches: [0; 2],
            output: 0,
            neighbors: vec![],
            publish_neighbors: None,
            adjacent_count: 0,
            allocations: vec![],
            nodes: vec![],
            can: [1; 6],
            existing: [false; 6],
            maximum: [1000.0; 6],
            costs: [10.0; 6],
            funds: [1000.0; 3],
            bases: [Some(1000.0); 3],
            minimum: Some(0.0),
            ratio: Some(1.0),
            created: vec![],
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn id(this: *mut u8) -> u32 {
    unsafe { word(this, 4) }
}
fn index(kind: u32) -> usize {
    kind.wrapping_sub(0x4773_FFE0) as usize
}

unsafe fn append(s: &mut State, header: *mut u8, object: u32) {
    unsafe {
        let previous = pointer(header, 4);
        let mut node = Box::new([header as u32, previous as u32, object]);
        let np = node.as_mut_ptr().cast::<u8>();
        previous.cast::<*mut u8>().write_unaligned(np);
        header.add(4).cast::<*mut u8>().write_unaligned(np);
        s.nodes.push(node);
    }
}

extern "thiscall" fn update(this: *mut u8, city: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.sim);
        assert_eq!(city as u32, 0x1020_3040);
        s.calls.push(Call::Update);
    });
    pair(0x9988_7766, 0x1234_5678)
}
extern "thiscall" fn current(_: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Current);
        pair(s.current as u32, 0x2233_4455)
    })
}
extern "thiscall" fn exemplar(_: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Exemplar);
        pair(s.exemplar as u32, 0x3344_5566)
    })
}
extern "thiscall" fn holder(_: *mut u8) -> u64 {
    record(Call::Holder);
    pair(0x2030_4050, 0x4455_6677)
}
extern "C" fn property(holder: u32, property: u32, out: *mut f32) -> u64 {
    assert_eq!(holder, 0x2030_4050);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Property(property, unsafe { out.read().to_bits() }));
        let value = match property {
            0xEA46_932C => s.bases[0],
            0x6A46_9337 => s.bases[1],
            0x8A46_933D => s.bases[2],
            0xCA46_9346 => None,
            0xEA4A_E9EF => s.minimum,
            0xCA46_931F => s.ratio,
            _ => unreachable!(),
        };
        if let Some(value) = value {
            unsafe { out.write(value) };
        }
    });
    pair(0x5566_7788, 0x6677_8899)
}
extern "C" fn framework() -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Framework);
        pair(s.exemplar as u32, 0x7788_99AA)
    })
}
extern "thiscall" fn service(_: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) -> u64 {
    assert_eq!((cls, iid), (0x3AE4_BEA3, 0x3AE4_BEAB));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Service);
        let i = usize::from(unsafe { (s.caches[0] as *const usize).read() } != 0);
        unsafe { out.write(s.allocators[i] as *mut u8) };
    });
    pair(1, 0x8899_AABB)
}
extern "thiscall" fn release_local(local: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::ReleaseLocal);
        let value = unsafe { local.read() } as usize;
        assert!(s
            .caches
            .iter()
            .any(|p| unsafe { (*p as *const usize).read() } == value));
    });
    pair(0x99AA_BBCC, 0xAABB_CCDD)
}
extern "thiscall" fn allocate(this: *mut u8, bytes: u32) -> u64 {
    assert_eq!(bytes, 12);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let deal = id(this) == 101;
        let n = s.allocations.len();
        s.calls.push(Call::Allocate(deal, n));
        let mut node = Box::new([0xCCCC_CCCC; 3]);
        let p = node.as_mut_ptr() as u32;
        s.allocations.push((deal, node));
        pair(p, 0xBBCC_DDEE)
    })
}
extern "thiscall" fn free(this: *mut u8, node: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let n = s
            .allocations
            .iter()
            .position(|(_, p)| p.as_ptr() as *mut u8 == node);
        let n = n.unwrap_or(usize::MAX);
        s.calls.push(Call::Free(id(this) == 101, n));
    });
    pair(0xCCDD_EEFF, 0xDDEE_FF00)
}
extern "thiscall" fn adjacent(_: *mut u8, list: *mut *mut u8, diagonals: u32) -> u64 {
    assert_eq!(diagonals, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Adjacent);
        let header = unsafe { list.read() };
        assert_eq!(unsafe { pointer(header, 0) }, header);
        assert_eq!(unsafe { pointer(header, 4) }, header);
        assert_eq!(unsafe { word(header, 8) }, 0xCCCC_CCCC);
        let cities = if s.adjacent_count == 0 {
            s.neighbors.clone()
        } else {
            s.publish_neighbors.clone().unwrap_or_else(|| s.neighbors.clone())
        };
        s.adjacent_count += 1;
        for city in cities {
            unsafe { append(&mut s, header, city as u32) };
        }
    });
    pair(0xDDEE_FF00, 0x1122_3344)
}
extern "thiscall" fn clear(list: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if list as usize == s.output {
            s.calls.push(Call::ClearOutput);
        } else {
            let n = s
                .allocations
                .iter()
                .position(|(_, p)| p.as_ptr() as u32 == unsafe { list.read() } as u32)
                .unwrap();
            s.calls.push(Call::Clear(n));
        }
        let header = unsafe { list.read() };
        unsafe {
            header.cast::<*mut u8>().write_unaligned(header);
            header.add(4).cast::<*mut u8>().write_unaligned(header);
        }
    });
    pair(0x1111_2222, 0x3333_4444)
}
extern "thiscall" fn can(_: *mut u8, kind: u32, city: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Can(kind, id(city)));
        pair(s.can[index(kind)], 0x4444_5555)
    })
}
extern "thiscall" fn find(_: *mut u8, kind: u32, city: *mut u8, list: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Find(kind, id(city)));
        if s.existing[index(kind)] {
            unsafe { append(&mut s, list.read(), 0x5555_6666) };
        }
    });
    pair(0x5555_6666, 0x6666_7777)
}
extern "thiscall" fn budget(this: *mut u8) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let i = id(this);
        s.calls.push(Call::Budget(i));
        s.funds[i as usize]
    })
}
extern "thiscall" fn maximum(_: *mut u8, kind: u32, city: *mut u8) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Max(kind, id(city)));
        s.maximum[index(kind)]
    })
}
extern "thiscall" fn cost(_: *mut u8, kind: u32, amount: f64, city: *mut u8) -> f64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Cost(kind, amount.to_bits(), id(city)));
        s.costs[index(kind)]
    })
}
extern "thiscall" fn create(
    _: *mut u8,
    kind: u32,
    flags: u32,
    city: *mut u8,
    output: *mut u8,
    add: u32,
) -> u64 {
    assert_eq!(flags, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(output as usize, s.output);
        s.calls.push(Call::Create(kind, id(city), add));
        s.created.push((kind, id(city), add));
        unsafe { append(&mut s, pointer(output, 0), kind) };
    });
    pair(1, 0x7777_8888)
}
extern "thiscall" fn erase(list: *mut *mut u8, output: *mut *mut u8, node: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        unsafe {
            let header = list.read();
            assert_eq!(pointer(header, 0), node);
            let object = pointer(node, 8);
            s.calls.push(Call::Erase(id(object)));
            let next = pointer(node, 0);
            header.cast::<*mut u8>().write_unaligned(next);
            next.add(4).cast::<*mut u8>().write_unaligned(header);
            output.write(next);
        }
    });
    pair(output as u32, 0x8888_9999)
}
extern "thiscall" fn publish(this: *mut u8, deals: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(deals as usize, s.sim + 0x20);
        assert_eq!(
            unsafe { NeighborsSim::from_ptr(s.sim as *mut u8) }.deals_dirty(),
            1
        );
        s.calls.push(Call::Publish(id(this)));
    });
    pair(0x9999_AAAA, 0xAAAA_BBBB)
}
extern "thiscall" fn figures(this: *mut u8) -> u64 {
    record(Call::Figures(id(this)));
    pair(0xBBBB_CCCC, 0xCCCC_DDDD)
}
extern "thiscall" fn release(this: *mut u8) -> u64 {
    record(Call::Release(id(this)));
    pair(0xDDDD_EEEE, 0xEEEE_FFFF)
}
extern "thiscall" fn destroy(list: *mut *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let header = unsafe { list.read() };
        assert_eq!(unsafe { pointer(header, 0) }, header);
        assert_eq!(unsafe { pointer(header, 4) }, header);
        let n = s
            .allocations
            .iter()
            .position(|(_, p)| p.as_ptr() as u32 == header as u32)
            .unwrap();
        if n != 0 {
            let sim = unsafe { NeighborsSim::from_ptr(s.sim as *mut u8) };
            assert_eq!((sim.deals_dirty(), sim.stats_dirty()), (0, 0));
        }
        s.calls.push(Call::DestroyCities(n));
    });
    pair(0xFACE_BA98, 0x7654_3210)
}

struct World {
    sim: Object,
    output: Box<*mut u8>,
    _output_header: Box<[u32; 3]>,
    _objects: [Box<[usize; 2]>; 7],
    _tables: [Box<[usize; 80]>; 4],
    data: Box<[usize; 5]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(neighbor_count: usize) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut tables: [Box<[usize; 80]>; 4] = core::array::from_fn(|_| Box::new([0; 80]));
        tables[0][0x88 / 4] = update as *const () as usize;
        tables[0][0x94 / 4] = exemplar as *const () as usize;
        tables[0][0x50 / 4] = adjacent as *const () as usize;
        tables[0][0x5C / 4] = can as *const () as usize;
        tables[0][0x74 / 4] = find as *const () as usize;
        tables[0][0x90 / 4] = maximum as *const () as usize;
        tables[0][0x8C / 4] = cost as *const () as usize;
        tables[1][0x90 / 4] = current as *const () as usize;
        tables[1][0x18 / 4] = holder as *const () as usize;
        tables[1][0x14 / 4] = service as *const () as usize;
        tables[2][0x0C / 4] = allocate as *const () as usize;
        tables[2][0x10 / 4] = free as *const () as usize;
        tables[3][0xC8 / 4] = budget as *const () as usize;
        tables[3][0x114 / 4] = publish as *const () as usize;
        tables[3][0x11C / 4] = figures as *const () as usize;
        tables[3][8 / 4] = release as *const () as usize;
        let mut objects: [Box<[usize; 2]>; 7] = core::array::from_fn(|i| {
            let (table, id) = match i {
                0..=2 => (3, i),
                3 => (2, 100),
                4 => (2, 101),
                _ => (1, 200 + i),
            };
            Box::new([tables[table].as_ptr() as usize, id])
        });
        let mut sim = Object::new(0x40);
        unsafe {
            sim.ptr().cast::<usize>().write(tables[0].as_ptr() as usize);
            let view = NeighborsSim::from_ptr(sim.ptr());
            view.set_initialized(1);
            view.set_stats_dirty(1);
        }
        let mut output_header = Box::new([0u32; 3]);
        let hp = output_header.as_mut_ptr().cast::<u8>();
        output_header[0] = hp as u32;
        output_header[1] = hp as u32;
        let mut output = Box::new(hp);
        let mut data = Box::new([
            0x1020_3040,
            objects[5].as_ptr() as usize,
            objects[3].as_ptr() as usize,
            objects[4].as_ptr() as usize,
            1.0f32.to_bits() as usize,
        ]);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = sim.ptr() as usize;
            s.current = objects[0].as_ptr() as usize;
            s.exemplar = objects[6].as_ptr() as usize;
            s.allocators = [data[2], data[3]];
            s.caches = [
                &mut data[2] as *mut usize as usize,
                &mut data[3] as *mut usize as usize,
            ];
            s.neighbors = objects[1..=neighbor_count]
                .iter_mut()
                .map(|o| o.as_mut_ptr() as usize)
                .collect();
            s.output = &mut *output as *mut *mut u8 as usize;
        });
        let fakes = vec![
            fake(0x0087_93EC, framework as *const () as usize),
            fake(0x0041_1870, release_local as *const () as usize),
            fake(0x005F_D4F0, property as *const () as usize),
            fake(0x0042_1660, clear as *const () as usize),
            fake(0x0041_1FE0, destroy as *const () as usize),
            fake(0x004A_4A50, erase as *const () as usize),
            fake(0x006D_0520, create as *const () as usize),
            fake_data(0x00B4_3CEC, (&mut data[0] as *mut usize).cast()),
            fake_data(0x00B4_3C94, (&mut data[1] as *mut usize).cast()),
            fake_data(0x00B2_07E0, (&mut data[2] as *mut usize).cast()),
            fake_data(0x00B2_0D58, (&mut data[3] as *mut usize).cast()),
            fake_data(0x00A8_1228, (&mut data[4] as *mut usize).cast()),
        ];
        Self {
            sim,
            output,
            _output_header: output_header,
            _objects: objects,
            _tables: tables,
            data,
            _fakes: fakes,
        }
    }
    fn call(&mut self, add: u32) -> (u64, Vec<Call>) {
        let mut args = [&mut *self.output as *mut *mut u8 as u32, add];
        let before = unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x40).to_vec() };
        STATE.with(|s| {
            s.borrow_mut().calls.clear();
        });
        let result = unsafe {
            checked_call(
                fc4p_006d21a0 as *const () as usize,
                self.sim.ptr(),
                &mut args,
                0xAABB_CCDD,
                0x1234_5678,
                Conv::Thiscall,
            )
        };
        let calls = STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls));
        let after = unsafe { std::slice::from_raw_parts(self.sim.ptr(), 0x40) };
        for offset in 0..0x40 {
            if offset != 0x3C && offset != 0x3D {
                assert_eq!(after[offset], before[offset], "simulator byte {offset:x}");
            }
        }
        (result, calls)
    }
}

fn prefix() -> Vec<Call> {
    vec![
        Call::Update,
        Call::ClearOutput,
        Call::Current,
        Call::Exemplar,
        Call::Holder,
        Call::Property(0xEA46_932C, 0),
        Call::Property(0x6A46_9337, 0),
        Call::Property(0x8A46_933D, 0),
        Call::Property(0xCA46_9346, 1.0f32.to_bits()),
        Call::Property(0xEA4A_E9EF, 0),
        Call::Property(0xCA46_931F, 1.0f32.to_bits()),
        Call::Allocate(false, 0),
        Call::Adjacent,
    ]
}

#[test]
fn uninitialized_preserves_output_and_integer_return_bytes() {
    let mut w = World::new(1);
    STATE.with(|s| unsafe { append(&mut s.borrow_mut(), *w.output, 0xABCD_EF01) });
    let first = unsafe { pointer(*w.output, 0) };
    unsafe {
        NeighborsSim::from_ptr(w.sim.ptr()).set_initialized(0);
    }
    let (result, calls) = w.call(1);
    assert_eq!(result, pair(0xAABB_CC01, 0x1234_5678));
    assert!(calls.is_empty());
    assert_eq!(unsafe { pointer(*w.output, 0) }, first);
}

#[test]
fn core_proposals_walk_cities_and_commodities_in_order_and_clean_every_temporary() {
    let mut w = World::new(2);
    let (result, calls) = w.call(0);
    assert_eq!(result, pair(0xFACE_BA01, 0x7654_3210));
    let mut expected = prefix();
    for city in 1..=2 {
        for commodity in 0..3 {
            let first = 1 + (city as usize - 1) * 6 + commodity * 2;
            expected.extend([Call::Allocate(true, first), Call::Allocate(true, first + 1)]);
            for direction in 0..2 {
                let kind = 0x4773_FFE0 + commodity as u32 * 2 + direction;
                expected.extend([
                    Call::Can(kind, city),
                    Call::Find(kind, city),
                    Call::Budget(if direction == 0 { 0 } else { city }),
                    Call::Max(kind, city),
                    Call::Cost(kind, 1000.0f64.to_bits(), city),
                    Call::Create(kind, city, 0),
                ]);
            }
            expected.extend([
                Call::Clear(first + 1),
                Call::Free(true, first + 1),
                Call::Clear(first),
                Call::Free(true, first),
            ]);
        }
        expected.push(Call::Erase(city));
    }
    expected.push(Call::DestroyCities(0));
    assert_eq!(calls, expected);
    STATE.with(|s| assert_eq!(s.borrow().created.len(), 12));
}

#[test]
fn core_ineligible_existing_and_small_maximum_each_short_circuit_at_the_right_call() {
    let mut w = World::new(1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.can = [0, 0x100, 1, 1, 1, 1];
        s.existing[1] = true;
        s.maximum[2] = 999.0;
        s.maximum[3] = f64::NAN;
        s.maximum[4] = 1001.0;
    });
    let (_, calls) = w.call(0);
    assert!(!calls.contains(&Call::Find(0x4773_FFE0, 1)));
    assert!(!calls.contains(&Call::Max(0x4773_FFE1, 1)));
    assert!(!calls
        .iter()
        .any(|c| matches!(c, Call::Cost(0x4773_FFE2 | 0x4773_FFE3, _, _))));
    assert!(calls.contains(&Call::Cost(0x4773_FFE4, 1000.0f64.to_bits(), 1)));
    STATE.with(|s| assert_eq!(s.borrow().created, [(0x4773_FFE4, 1, 0), (0x4773_FFE5, 1, 0)]));
}

#[test]
fn core_affordability_uses_three_strict_tests_and_the_buyers_budget() {
    for (funds, floor, ratio, cost, want) in [
        (100.0, 100.0, 1.0, 10.0, false),
        (100.0, 0.0, 1.0, 100.0, false),
        (100.0, 0.0, 10.0, 10.0, false),
        (100.0, 0.0, 9.0, 10.0, true),
        (100.0, f32::NAN, 1.0, 10.0, false),
        (100.0, 0.0, f32::NAN, 10.0, false),
        (100.0, 0.0, 1.0, f64::NAN, false),
        (f64::NAN, 0.0, 1.0, 10.0, false),
    ] {
        let mut w = World::new(1);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.can = [1, 1, 0, 0, 0, 0];
            s.funds = [funds, 1000.0, 0.0];
            s.minimum = Some(floor);
            s.ratio = Some(ratio);
            s.costs = [cost; 6];
        });
        let (_, calls) = w.call(0);
        let buy = calls.contains(&Call::Create(0x4773_FFE0, 1, 0));
        assert_eq!(buy, want);
        assert!(calls.contains(&Call::Budget(0)));
        assert!(calls.contains(&Call::Budget(1)));
    }
}

#[test]
fn adding_nonempty_proposals_publishes_back_to_front_then_current_and_clears_both_dirty_bytes() {
    let mut w = World::new(2);
    let (_, calls) = w.call(0xAABB_CC01);
    let start = calls.iter().rposition(|c| *c == Call::Adjacent).unwrap();
    assert_eq!(
        &calls[start..],
        &[
            Call::Adjacent,
            Call::Current,
            Call::Publish(2),
            Call::Figures(2),
            Call::Release(2),
            Call::Free(false, usize::MAX),
            Call::Publish(1),
            Call::Figures(1),
            Call::Release(1),
            Call::Free(false, usize::MAX),
            Call::Publish(0),
            Call::Figures(0),
            Call::DestroyCities(13),
            Call::DestroyCities(0),
        ]
    );
    let sim = unsafe { NeighborsSim::from_ptr(w.sim.ptr()) };
    assert_eq!((sim.deals_dirty(), sim.stats_dirty()), (0, 0));
    STATE.with(|s| assert!(s.borrow().created.iter().all(|c| c.2 == 0xAABB_CC01)));
}

#[test]
fn empty_output_and_zero_add_byte_skip_publication() {
    for add in [0, 0xFFFF_FF00, 1] {
        let mut w = World::new(1);
        STATE.with(|s| s.borrow_mut().can = [0; 6]);
        let (_, calls) = w.call(add);
        assert!(!calls.iter().any(|c| matches!(c, Call::Publish(_))));
        assert_eq!(calls.iter().filter(|c| **c == Call::Adjacent).count(), 1);
        assert_eq!(unsafe { NeighborsSim::from_ptr(w.sim.ptr()) }.stats_dirty(), 1);
    }
}

#[test]
fn missing_tuning_keeps_defaults_and_zero_base_is_an_eligible_proposal() {
    let mut w = World::new(1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.bases = [None; 3];
        s.minimum = None;
        s.ratio = None;
    });
    let (_, calls) = w.call(0);
    for kind in 0x4773_FFE0..=0x4773_FFE5 {
        assert!(calls.contains(&Call::Cost(kind, 0.0f64.to_bits(), 1)));
    }
    STATE.with(|s| assert_eq!(s.borrow().created.len(), 6));
}

#[test]
fn both_allocator_caches_are_initialized_once_and_no_neighbors_still_clears_output() {
    let mut w = World::new(1);
    w.data[2] = 0;
    w.data[3] = 0;
    let (_, calls) = w.call(0);
    assert_eq!(calls.iter().filter(|c| **c == Call::Framework).count(), 2);
    for i in calls
        .iter()
        .enumerate()
        .filter_map(|(i, c)| if *c == Call::Framework { Some(i) } else { None })
    {
        assert_eq!(
            &calls[i..i + 3],
            &[Call::Framework, Call::Service, Call::ReleaseLocal]
        );
    }
    drop(w);
    let mut w = World::new(0);
    let (_, calls) = w.call(1);
    let mut expected = prefix();
    expected.push(Call::DestroyCities(0));
    assert_eq!(calls, expected);
}

#[test]
fn nonempty_output_with_zero_add_byte_keeps_proposals_without_publishing() {
    let mut w = World::new(1);
    let (_, calls) = w.call(0xFFFF_FF00);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(s.created.len(), 6);
        assert!(s.created.iter().all(|c| c.2 == 0xFFFF_FF00));
    });
    assert!(!calls.iter().any(|c| matches!(c, Call::Publish(_))));
    assert_eq!(calls.iter().filter(|c| **c == Call::Adjacent).count(), 1);
}

#[test]
fn publication_skips_null_cities_and_still_unlinks_their_list_entries() {
    let mut w = World::new(1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.publish_neighbors = Some(vec![s.neighbors[0], 0]);
    });
    let (_, calls) = w.call(1);
    let start = calls.iter().rposition(|c| *c == Call::Adjacent).unwrap();
    assert_eq!(
        &calls[start..],
        &[
            Call::Adjacent,
            Call::Current,
            Call::Free(false, usize::MAX),
            Call::Publish(1),
            Call::Figures(1),
            Call::Release(1),
            Call::Free(false, usize::MAX),
            Call::Publish(0),
            Call::Figures(0),
            Call::DestroyCities(7),
            Call::DestroyCities(0),
        ]
    );
}
