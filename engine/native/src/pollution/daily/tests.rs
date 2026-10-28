// SPDX-License-Identifier: GPL-3.0-or-later
//! Native-layout daily vectors with recorded service calls.

// Every pointee must keep its address while the owning vector grows.
#![allow(clippy::vec_box)]

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;
use std::collections::{BTreeMap, VecDeque};

const RESULT: u64 = 0x1234_ABCD_5678_EF90;
const CLEAR_RESULT: u64 = 0xAABB_CCDD_1122_3344;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Framework,
    Acquire(u32, u32),
    ReleaseAllocator,
    Capacity(u32, u32),
    Holder(u32),
    Property(u32, u32),
    Allocate(u32),
    Find(u32),
    DealUsed(u32),
    DealAmount(u32),
    SetDeal(u32, i32),
    Dispose(u32, u32, u32),
    Supply(i32),
    Factor(u32, u32),
    Source(u32, i32, i32),
    Zone(i32, i32),
    Get(u32, i32, i32),
    Set(u32, i32, i32, i32),
    TrafficGrid,
    Traffic(i32, i32),
    Dimension(u32),
    Reset,
    EndCycle,
    Next,
    Distribute(usize, u32, u32, u32, u32, u32),
    Maintenance(usize),
    Clear,
    Free,
}

#[repr(C)]
struct Node {
    next: *mut u8,
    previous: *mut u8,
    occupant: *mut u8,
}
#[repr(C)]
struct Interface {
    table: *const usize,
    id: u32,
}
#[repr(C)]
struct Deal {
    table: *const usize,
    id: u32,
    amount: i32,
    used: i32,
}

struct State {
    calls: Vec<Call>,
    sim: *mut u8,
    sentinel: *mut u8,
    traffic: *mut u8,
    allocator: *mut u8,
    allocator_global: *mut *mut u8,
    framework: *mut u8,
    cold_after_clear: bool,
    capacities: BTreeMap<u32, VecDeque<Option<u32>>>,
    properties: BTreeMap<u32, Option<u32>>,
    factors: BTreeMap<(u32, u32), f64>,
    source: [i32; 4],
    zones: VecDeque<u32>,
    zone: u32,
    values: BTreeMap<(u32, i32, i32), i32>,
    traffic_value: u32,
    dispose: VecDeque<(f64, Option<(u32, u32)>)>,
    coordinates: VecDeque<(i32, i32)>,
    deals: Vec<Box<Deal>>,
    deal_nodes: Vec<Box<Node>>,
    deal_table: *const usize,
    imports: Vec<(i32, i32)>,
    exports: Vec<(i32, i32)>,
    dimensions: (u32, u32),
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            sim: core::ptr::null_mut(),
            sentinel: core::ptr::null_mut(),
            allocator: core::ptr::null_mut(),
            allocator_global: core::ptr::null_mut(),
            framework: core::ptr::null_mut(),
            cold_after_clear: false,
            traffic: core::ptr::null_mut(),
            capacities: BTreeMap::new(),
            properties: BTreeMap::new(),
            factors: BTreeMap::new(),
            source: [0; 4],
            zones: VecDeque::new(),
            zone: 0,
            values: BTreeMap::new(),
            traffic_value: 0,
            dispose: VecDeque::new(),
            coordinates: VecDeque::new(),
            deals: vec![],
            deal_nodes: vec![],
            deal_table: core::ptr::null(),
            imports: vec![],
            exports: vec![],
            dimensions: (8, 8),
        }
    }
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(call: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
fn edit(f: impl FnOnce(&mut State)) {
    STATE.with(|s| f(&mut s.borrow_mut()));
}

extern "thiscall" fn capacity(_this: *mut u8, property: u32, occupant: *mut Interface, output: *mut u32) -> u32 {
    let id = unsafe { (*occupant).id };
    log(Call::Capacity(id, property));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let value = s
            .capacities
            .get_mut(&id)
            .and_then(|v| v.pop_front())
            .unwrap_or(Some(100));
        if let Some(v) = value {
            unsafe { output.write(v) };
            0x9876_5401
        } else {
            0x9876_5400
        }
    })
}
extern "thiscall" fn holder(this: *mut Interface) -> *mut Interface {
    log(Call::Holder(unsafe { (*this).id }));
    this
}
extern "C" fn property(holder: *mut Interface, property: u32, output: *mut u32) -> u32 {
    let id = unsafe { (*holder).id };
    log(Call::Property(id, property));
    assert_eq!(unsafe { output.read() }, 0);
    STATE.with(|s| {
        if let Some(value) = s.borrow().properties.get(&id).copied().unwrap_or(Some(0)) {
            unsafe { output.write(value) };
            0x1357_2401
        } else {
            0x1357_2400
        }
    })
}
extern "thiscall" fn allocate(_this: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().sentinel)
}
extern "C" fn framework() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().framework)
}
extern "thiscall" fn acquire(_this: *mut u8, cls: u32, iid: u32, out: *mut *mut u8) {
    log(Call::Acquire(cls, iid));
    STATE.with(|s| unsafe { out.write(s.borrow().allocator) });
}
extern "fastcall" fn release_allocator(pointer: *mut *mut u8, edx: u32) -> u64 {
    log(Call::ReleaseAllocator);
    assert_eq!(unsafe { pointer.read() } as u32, edx);
    registers(0, (CLEAR_RESULT >> 32) as u32)
}
extern "fastcall" fn free(_this: *mut u8, edx: u32, pointer: *mut u8) -> u64 {
    log(Call::Free);
    assert_eq!(edx, (CLEAR_RESULT >> 32) as u32);
    STATE.with(|s| assert_eq!(pointer, s.borrow().sentinel));
    RESULT
}
extern "thiscall" fn find(_this: *mut u8, id: u32, list: *mut *mut u8) {
    log(Call::Find(id));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let amounts = if id == 0x4773_FFE5 {
            s.imports.clone()
        } else {
            s.exports.clone()
        };
        let sentinel = unsafe { list.read() };
        // This service replaces the previous search results.
        unsafe {
            let n = ListNode::from_ptr(sentinel);
            n.set_next(sentinel);
            n.set_previous(sentinel);
        }
        for (i, (amount, used)) in amounts.into_iter().enumerate() {
            let id = (if id == 0x4773_FFE5 { 100 } else { 200 }) + i as u32;
            let mut deal = Box::new(Deal {
                table: s.deal_table,
                id,
                amount,
                used,
            });
            let mut node = Box::new(Node {
                next: sentinel,
                previous: unsafe { ListNode::from_ptr(sentinel).previous() },
                occupant: (&mut *deal as *mut Deal).cast(),
            });
            let pointer = (&mut *node as *mut Node).cast();
            unsafe {
                ListNode::from_ptr(node.previous).set_next(pointer);
                ListNode::from_ptr(sentinel).set_previous(pointer);
            }
            s.deal_nodes.push(node);
            s.deals.push(deal);
        }
    });
}
extern "thiscall" fn deal_used(this: *mut Deal) -> i32 {
    log(Call::DealUsed(unsafe { (*this).id }));
    unsafe { (*this).used }
}
extern "thiscall" fn deal_amount(this: *mut Deal) -> i32 {
    log(Call::DealAmount(unsafe { (*this).id }));
    unsafe { (*this).amount }
}
extern "thiscall" fn deal_set(this: *mut Deal, value: i32) {
    log(Call::SetDeal(unsafe { (*this).id }, value));
    unsafe {
        (*this).used = value;
    }
}
extern "thiscall" fn dispose(_this: *mut u8, offered: f32, inc: *mut u32, energy: *mut u32) -> f64 {
    log(Call::Dispose(offered.to_bits(), unsafe { inc.read() }, unsafe {
        energy.read()
    }));
    STATE.with(|s| {
        let result = s.borrow_mut().dispose.pop_front().unwrap_or((0.0, None));
        if let Some((i, e)) = result.1 {
            unsafe {
                inc.write(i);
                energy.write(e);
            }
        }
        result.0
    })
}
extern "thiscall" fn supply(this: *mut u8, delta: i32) {
    log(Call::Supply(delta));
    let sim = unsafe { PollutionSim::from_ptr(this) };
    sim.set_supply(sim.supply().wrapping_add(delta));
}
extern "thiscall" fn factor(_this: *mut u8, id: u32, zone: u32) -> f64 {
    log(Call::Factor(id, zone));
    STATE.with(|s| s.borrow().factors.get(&(id, zone)).copied().unwrap_or(1.0))
}
extern "thiscall" fn source(this: *mut Interface, x: i32, z: i32) -> i32 {
    let id = unsafe { (*this).id };
    log(Call::Source(id, x, z));
    STATE.with(|s| s.borrow().source[id as usize])
}
extern "thiscall" fn zone(_this: *mut u8, x: i32, z: i32) -> u32 {
    log(Call::Zone(x, z));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.zones.pop_front().unwrap_or(s.zone)
    })
}
fn get_value(kind: u32, x: i32, z: i32, out: *mut i32) {
    log(Call::Get(kind, x, z));
    let v = STATE.with(|s| s.borrow().values.get(&(kind, x, z)).copied().unwrap_or(0));
    unsafe {
        out.write(v);
    }
}
extern "thiscall" fn get_air(_this: *mut u8, x: i32, z: i32, out: *mut i32) {
    get_value(0, x, z, out);
}
extern "thiscall" fn get_water(_this: *mut u8, x: i32, z: i32, out: *mut i32) {
    get_value(1, x, z, out);
}
extern "thiscall" fn get_garbage(_this: *mut u8, x: i32, z: i32, out: *mut i32) {
    get_value(2, x, z, out);
}
fn set_value(kind: u32, x: i32, z: i32, value: i32) {
    log(Call::Set(kind, x, z, value));
    edit(|s| {
        s.values.insert((kind, x, z), value);
    });
}
extern "thiscall" fn set_air(_this: *mut u8, x: i32, z: i32, value: i32) {
    set_value(0, x, z, value);
}
extern "thiscall" fn set_water(_this: *mut u8, x: i32, z: i32, value: i32) {
    set_value(1, x, z, value);
}
extern "thiscall" fn set_garbage(_this: *mut u8, x: i32, z: i32, value: i32) {
    set_value(2, x, z, value);
}
extern "thiscall" fn traffic_grid(_this: *mut u8) -> *mut u8 {
    log(Call::TrafficGrid);
    STATE.with(|s| s.borrow().traffic)
}
extern "thiscall" fn traffic(_this: *mut u8, x: i32, z: i32) -> u32 {
    log(Call::Traffic(x, z));
    STATE.with(|s| s.borrow().traffic_value)
}
extern "thiscall" fn width(_this: *mut u8) -> u32 {
    log(Call::Dimension(0));
    STATE.with(|s| s.borrow().dimensions.0)
}
extern "thiscall" fn height(_this: *mut u8) -> u32 {
    log(Call::Dimension(1));
    STATE.with(|s| s.borrow().dimensions.1)
}
extern "thiscall" fn reset(_this: *mut u8, shuffle: u32) {
    assert_eq!(shuffle, 1);
    log(Call::Reset);
}
extern "thiscall" fn end_cycle(_this: *mut u8) {
    log(Call::EndCycle);
}
extern "thiscall" fn next(_this: *mut u8, x: *mut i32, z: *mut i32) {
    log(Call::Next);
    let pair = STATE.with(|s| s.borrow_mut().coordinates.pop_front().unwrap_or((1, 2)));
    unsafe {
        x.write(pair.0);
        z.write(pair.1);
    }
}
extern "thiscall" fn distribute(
    this: *mut u8,
    list: *mut u8,
    capacity: u32,
    used: u32,
    total: u32,
    amount: *mut u32,
    count: u32,
) {
    log(Call::Distribute(
        (list as usize).wrapping_sub(this as usize),
        capacity,
        used,
        total,
        unsafe { amount.read() },
        count,
    ));
}
extern "thiscall" fn maintenance(this: *mut u8, list: *mut u8) {
    log(Call::Maintenance((list as usize).wrapping_sub(this as usize)));
}
extern "thiscall" fn clear(_list: *mut *mut u8) -> u64 {
    log(Call::Clear);
    STATE.with(|s| {
        let s = s.borrow();
        if s.cold_after_clear {
            unsafe {
                s.allocator_global.write(core::ptr::null_mut());
            }
        }
    });
    CLEAR_RESULT
}

struct World {
    object: Object,
    _interfaces: Vec<Box<Interface>>,
    _tables: Vec<Box<[usize; 64]>>,
    lists: Vec<Box<Node>>,
    _allocator_global: Box<*mut u8>,
    constants: Vec<Box<u64>>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}
impl World {
    fn new() -> Self {
        edit(|s| *s = State::default());
        let mut tables = vec![];
        let mut interfaces = vec![];
        for (id, slots) in [
            (
                0,
                vec![
                    (0x1C, get_air as *const () as usize),
                    (0x20, get_water as *const () as usize),
                    (0x24, get_garbage as *const () as usize),
                    (0x68, set_air as *const () as usize),
                    (0x6C, set_water as *const () as usize),
                    (0x70, set_garbage as *const () as usize),
                    (0x74, supply as *const () as usize),
                    (0xD4, capacity as *const () as usize),
                ],
            ),
            (
                0,
                vec![
                    (0x0C, allocate as *const () as usize),
                    (0x10, free as *const () as usize),
                ],
            ),
            (0, vec![(0x78, find as *const () as usize)]),
            (0, vec![(0x64, factor as *const () as usize)]),
            (0, vec![(0x30, zone as *const () as usize)]),
            (0, vec![(0x20, traffic_grid as *const () as usize)]),
            (0, vec![(0x1C, traffic as *const () as usize)]),
            (
                0,
                vec![
                    (0x1C, source as *const () as usize),
                    (0x30, width as *const () as usize),
                    (0x34, height as *const () as usize),
                ],
            ),
            (1, vec![(0x1C, source as *const () as usize)]),
            (2, vec![(0x1C, source as *const () as usize)]),
            (3, vec![(0x1C, source as *const () as usize)]),
            (0, vec![(0x18, holder as *const () as usize)]),
            (
                0,
                vec![
                    (0x28, deal_amount as *const () as usize),
                    (0x50, deal_used as *const () as usize),
                    (0x54, deal_set as *const () as usize),
                ],
            ),
            (0, vec![(0x14, acquire as *const () as usize)]),
        ] {
            let mut table = Box::new([0; 64]);
            for (slot, f) in slots {
                table[slot / 4] = f;
            }
            interfaces.push(Box::new(Interface {
                table: table.as_ptr(),
                id,
            }));
            tables.push(table);
        }
        let pointers: Vec<*mut u8> = interfaces
            .iter_mut()
            .map(|o| (&mut **o as *mut Interface).cast())
            .collect();
        let mut object = Object::new(0x240);
        let base = object.ptr();
        let sim = unsafe { DailyView::from_ptr(base) };
        let common = unsafe { PollutionSim::from_ptr(base) };
        common.set_primary_vtable(tables[0].as_ptr() as u32);
        sim.set_neighbors(pointers[2]);
        sim.set_ordinances(pointers[3]);
        common.set_zones(pointers[4]);
        sim.set_traffic(pointers[5]);
        common.set_air_source(pointers[7]);
        common.set_water_source(pointers[8]);
        common.set_garbage_source(pointers[9]);
        common.set_radiation_source(pointers[10]);
        common.set_capacity(1_000_000);
        let mut lists = vec![];
        for offset in [0x68usize, 0x6C, 0x70, 0x74] {
            let mut node = Box::new(Node {
                next: core::ptr::null_mut(),
                previous: core::ptr::null_mut(),
                occupant: core::ptr::null_mut(),
            });
            let ptr: *mut u8 = (&mut *node as *mut Node).cast();
            node.next = ptr;
            node.previous = ptr;
            unsafe {
                base.add(offset).cast::<*mut u8>().write_unaligned(ptr);
            }
            lists.push(node);
        }
        let mut sentinel = Box::new(Node {
            next: core::ptr::null_mut(),
            previous: core::ptr::null_mut(),
            occupant: 0x1234usize as *mut u8,
        });
        let sentinel_ptr = (&mut *sentinel as *mut Node).cast();
        lists.push(sentinel);
        let mut allocator_global = Box::new(pointers[1]);
        let mut fakes: Vec<_> = [
            (0x005F_D480, property as *const () as usize),
            (0x006E_3820, dispose as *const () as usize),
            (0x006E_3FE0, distribute as *const () as usize),
            (0x006E_4100, maintenance as *const () as usize),
            (0x0049_D1C0, reset as *const () as usize),
            (0x0049_CB10, next as *const () as usize),
            (0x006E_7930, end_cycle as *const () as usize),
            (0x0042_1660, clear as *const () as usize),
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_allocator as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        fakes.push(fake_data(0x00B2_0D58, (&mut *allocator_global as *mut *mut u8).cast()));
        let mut constants = vec![];
        for (address, bits) in [
            (0x00A8_1054, 0.0f32.to_bits()),
            (0x00A8_1228, 1.0f32.to_bits()),
            (0x00AB_2550, 0.1f32.to_bits()),
            (0x00AB_2554, 12_582_912.0f32.to_bits()),
            (0x00AB_274C, 32767.0f32.to_bits()),
            (0x00A8_0AA8, 4_294_967_296.0f32.to_bits()),
            (0x00B4_9B8C, 1.0f32.to_bits()),
            (0x00B4_9BC4, 0.0f32.to_bits()),
            (0x00B4_9B88, 100),
            (0x00B4_9B84, 100),
            (0x00B4_9B68, 100),
        ] {
            let mut value = Box::new(u64::from(bits));
            fakes.push(fake_data(address, (&mut *value as *mut u64).cast()));
            constants.push(value);
        }
        edit(|s| {
            s.sim = base;
            s.sentinel = sentinel_ptr;
            s.traffic = pointers[6];
            s.deal_table = tables[12].as_ptr();
            s.allocator = pointers[1];
            s.allocator_global = &mut *allocator_global;
            s.framework = pointers[13];
        });
        Self {
            object,
            _interfaces: interfaces,
            _tables: tables,
            lists,
            _allocator_global: allocator_global,
            constants,
            _fakes: fakes,
            _fpu: GameFpu::new(),
        }
    }
    fn sim(&mut self) -> DailyView {
        unsafe { DailyView::from_ptr(self.object.ptr()) }
    }
    fn common(&mut self) -> PollutionSim {
        unsafe { PollutionSim::from_ptr(self.object.ptr()) }
    }
    fn run(&mut self) -> u64 {
        unsafe { fc4p_006e7fd0(self.object.ptr()) }
    }
    fn append(&mut self, list: usize, id: u32) {
        let mut occupant = Box::new(Interface {
            table: self._tables[11].as_ptr(),
            id,
        });
        let end: *mut u8 = (&mut *self.lists[list] as *mut Node).cast();
        let mut node = Box::new(Node {
            next: end,
            previous: self.lists[list].previous,
            occupant: (&mut *occupant as *mut Interface).cast(),
        });
        let ptr = (&mut *node as *mut Node).cast();
        unsafe {
            ListNode::from_ptr(node.previous).set_next(ptr);
            ListNode::from_ptr(end).set_previous(ptr);
        }
        self.lists.push(node);
        self._interfaces.push(occupant);
    }
}

fn factors_calls() -> Vec<Call> {
    let mut v: Vec<_> = [0x8A67_E378, 0x08F7_9B8E, 0xE8F7_9C8B, 0xE8F7_9C90]
        .into_iter()
        .map(|id| Call::Factor(id, 0))
        .collect();
    for zone in 0..16 {
        for id in [0x8A67_E373, 0x8A67_E374, 0x8A67_E376] {
            v.push(Call::Factor(id, zone));
        }
    }
    v
}

#[test]
fn empty_day_keeps_unrelated_bytes_and_performs_every_factor_and_maintenance_call() {
    let mut w = World::new();
    let sim = w.sim();
    sim.set_incinerator_capacity(99);
    sim.set_energy_capacity(88);
    let before = unsafe { std::slice::from_raw_parts(w.object.ptr(), 0x240).to_vec() };
    assert_eq!(w.run(), RESULT);
    let mut expected = before;
    expected[0x12C..0x134].fill(0);
    assert_eq!(unsafe { std::slice::from_raw_parts(w.object.ptr(), 0x240) }, expected);
    let mut calls = vec![
        Call::Allocate(12),
        Call::Find(0x4773_FFE5),
        Call::Find(0x4773_FFE4),
        Call::Dispose(0, 0, 0),
        Call::Supply(0),
    ];
    calls.extend(factors_calls());
    calls.extend([
        Call::Maintenance(0x68),
        Call::Maintenance(0x6C),
        Call::Maintenance(0x74),
        Call::Clear,
        Call::Free,
    ]);
    assert_eq!(take(), calls);
}

#[test]
fn capacities_repeat_queries_handle_failures_wrap_totals_and_distribute_the_used_difference() {
    let mut w = World::new();
    for (list, id) in [(0, 1), (0, 2), (0, 3), (0, 4), (1, 5), (1, 6), (1, 7), (2, 8)] {
        w.append(list, id);
    }
    edit(|s| {
        for (id, queries, used) in [
            (1, vec![Some(100)], Some(20)),
            (2, vec![None], Some(0)),
            (3, vec![Some(5)], Some(10)),
            (4, vec![Some(u32::MAX)], Some(0)),
            (5, vec![Some(200), Some(150)], Some(50)),
            (6, vec![None, Some(50)], None),
            (7, vec![Some(u32::MAX), Some(10)], Some(0)),
            (8, vec![Some(300), Some(250)], Some(20)),
        ] {
            s.capacities.insert(id, queries.into());
            s.properties.insert(id, used);
        }
        s.dispose.push_back((0.0, Some((70, 200))));
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!((w.sim().incinerator_capacity(), w.sim().energy_capacity()), (199, 300));
    let calls = take();
    let prefix = [
        Call::Capacity(1, 0xC8EB_D131),
        Call::Holder(1),
        Call::Property(1, 0x28EC_F510),
        Call::Capacity(2, 0xC8EB_D131),
        Call::Capacity(3, 0xC8EB_D131),
        Call::Holder(3),
        Call::Property(3, 0x28EC_F510),
        Call::Capacity(4, 0xC8EB_D131),
        Call::Holder(4),
        Call::Property(4, 0x28EC_F510),
        Call::Capacity(5, 0xA8EB_CF0C),
        Call::Capacity(5, 0xA8EB_CF0C),
        Call::Holder(5),
        Call::Property(5, 0xE8EC_F641),
        Call::Capacity(6, 0xA8EB_CF0C),
        Call::Capacity(6, 0xA8EB_CF0C),
        Call::Holder(6),
        Call::Property(6, 0xE8EC_F641),
        Call::Capacity(7, 0xA8EB_CF0C),
        Call::Capacity(7, 0xA8EB_CF0C),
        Call::Holder(7),
        Call::Property(7, 0xE8EC_F641),
        Call::Capacity(8, 0x48EB_D0B1),
        Call::Capacity(8, 0x48EB_D0B1),
        Call::Holder(8),
        Call::Property(8, 0x88EC_F76C),
    ];
    assert_eq!(&calls[..prefix.len()], prefix);
    assert!(calls.contains(&Call::Dispose(0, 110, 230)));
    let distribution: Vec<_> = calls
        .into_iter()
        .filter(|c| matches!(c, Call::Distribute(..)))
        .collect();
    assert_eq!(
        distribution,
        [
            Call::Distribute(0x6C, 0xA8EB_CF0C, 0xE8EC_F641, 110, 40, 3),
            Call::Distribute(0x70, 0x48EB_D0B1, 0x88EC_F76C, 230, 30, 1)
        ]
    );
}

#[test]
fn deals_reread_amounts_and_usage_round_imports_and_limit_exports_to_room() {
    let mut w = World::new();
    w.sim().set_imported(-7);
    w.sim().set_export_capacity(10);
    edit(|s| {
        s.imports = vec![(1000, 900), (100, 100), (50, 49)];
        s.exports = vec![(1000, 995), (80, 0), (1, 1)];
        s.dispose.push_back((4.75, None));
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!(w.sim().imported(), 98);
    assert_eq!(w.sim().export_capacity(), 23);
    assert_eq!(w.common().supply(), 100);
    let calls = take();
    assert_eq!(
        &calls[..26],
        [
            Call::Allocate(12),
            Call::Find(0x4773_FFE5),
            Call::DealUsed(100),
            Call::DealAmount(100),
            Call::DealAmount(100),
            Call::DealUsed(100),
            Call::SetDeal(100, 1000),
            Call::DealUsed(101),
            Call::DealAmount(101),
            Call::DealUsed(102),
            Call::DealAmount(102),
            Call::DealAmount(102),
            Call::DealUsed(102),
            Call::SetDeal(102, 54),
            Call::Find(0x4773_FFE4),
            Call::DealUsed(200),
            Call::DealAmount(200),
            Call::DealAmount(200),
            Call::DealUsed(200),
            Call::SetDeal(200, 1000),
            Call::DealUsed(201),
            Call::DealAmount(201),
            Call::DealAmount(201),
            Call::DealUsed(201),
            Call::SetDeal(201, 8),
            Call::DealUsed(202)
        ]
    );
    assert!(calls.contains(&Call::Dispose(105.0f32.to_bits(), 0, 0)));
    assert!(calls.contains(&Call::Supply(100)));
}

#[test]
fn one_tract_applies_traffic_recycling_and_relaxation_and_advances_the_iterator() {
    let mut w = World::new();
    let sim = w.sim();
    let common = w.common();
    sim.set_visits(1);
    common.set_recycling_effect(0.25);
    edit(|s| {
        s.source = [100, 200, 400, 2];
        s.zone = 1;
        s.traffic_value = 0x1108;
    });
    let before = unsafe { std::slice::from_raw_parts(w.object.ptr(), 0x240).to_vec() };
    assert_eq!(w.run(), RESULT);
    let after = unsafe { std::slice::from_raw_parts(w.object.ptr(), 0x240) };
    for (offset, (a, b)) in after.iter().zip(&before).enumerate() {
        if ![
            (0x60..0x68),
            (0x108..0x114),
            (0x120..0x128),
            (0x13C..0x148),
            (0x158..0x160),
        ]
        .iter()
        .any(|r| r.contains(&offset))
        {
            assert_eq!(a, b, "write at +{offset:#x}");
        }
    }
    assert_eq!(
        (sim.air_sum(), sim.water_sum(), sim.treated_water(), sim.radiation_sum()),
        (132, 200, 200, 2)
    );
    assert_eq!(
        (sim.produced(), sim.recycled(), common.uncollected(), sim.developed()),
        (400, 100.0, 300, 1)
    );
    assert_eq!((sim.x(), sim.z()), (2, 4));
    let calls = take();
    let start = calls.iter().position(|c| matches!(c, Call::Source(..))).unwrap();
    let mut expected = vec![
        Call::Source(0, 0, 0),
        Call::Source(1, 0, 0),
        Call::Source(2, 0, 0),
        Call::Source(3, 0, 0),
        Call::Zone(0, 0),
        Call::Zone(1, 0),
        Call::Zone(0, 1),
        Call::Zone(1, 1),
    ];
    for (x, z) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
        expected.extend([
            Call::Zone(x, z),
            Call::Get(2, x, z),
            Call::TrafficGrid,
            Call::Traffic(x, z),
            Call::Set(2, x, z, 0),
            Call::Dispose(75.0f32.to_bits(), 0, 0),
            Call::Set(2, x, z, 75),
        ]);
    }
    expected.extend([
        Call::Get(0, 0, 0),
        Call::Set(0, 0, 0, 100),
        Call::Get(1, 0, 0),
        Call::Set(1, 0, 0, 100),
        Call::Next,
        Call::Maintenance(0x68),
        Call::Maintenance(0x6C),
        Call::Maintenance(0x74),
        Call::Clear,
        Call::Free,
    ]);
    assert_eq!(&calls[start..], expected);
}

#[test]
fn landfill_cells_skip_garbage_but_still_receive_traffic_and_signed_word_narrowing() {
    let mut w = World::new();
    let sim = w.sim();
    sim.set_visits(1);
    edit(|s| {
        s.source = [32767, -1, -100, -1];
        s.zone = 14;
        s.factors.insert((0x08F7_9B8E, 0), 2.0);
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!(
        (
            sim.air_sum(),
            sim.water_sum(),
            sim.radiation_sum(),
            sim.produced(),
            sim.developed()
        ),
        (-2, 0, 0, 0, 0)
    );
    let calls = take();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Traffic(..))).count(), 4);
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Dispose(..))).count(), 1);
    assert!(!calls.iter().any(|c| matches!(c, Call::Set(..))));
}

#[test]
fn zone_queries_are_repeated_and_zone_factors_are_averaged_in_the_original_order() {
    let mut w = World::new();
    let sim = w.sim();
    sim.set_visits(1);
    edit(|s| {
        s.source = [300, 300, 300, 0];
        s.zones = [1, 2, 14, 0, 14, 14, 14, 14].into();
        for id in [0x8A67_E373, 0x8A67_E374, 0x8A67_E376] {
            s.factors.insert((id, 1), 0.5);
            s.factors.insert((id, 2), 2.0);
        }
    });
    assert_eq!(w.run(), RESULT);
    // (0.5 + 2 + 1) * (1/3), followed by signed-word multiplication and truncation.
    assert_eq!((sim.air_sum(), sim.water_sum(), sim.developed()), (350, 350, 1));
    assert_eq!(sim.produced(), 0);
    let calls = take();
    assert_eq!(calls.iter().filter(|c| matches!(c, Call::Zone(..))).count(), 8);
}

#[test]
fn water_treatment_uses_previous_ratio_and_minimum_one_then_distributes_actual_usage() {
    let mut w = World::new();
    w.append(0, 1);
    let sim = w.sim();
    sim.set_visits(1);
    sim.set_previous_water(1000);
    sim.set_previous_treated(100);
    edit(|s| {
        s.source = [0, 1, 0, 0];
        s.zone = 14;
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!((sim.water_sum(), sim.treatment_sum(), sim.treated_water()), (1, 1, 0));
    assert!(take().contains(&Call::Distribute(0x68, 0xC8EB_D131, 0x28EC_F510, 100, 1, 1)));
}

#[test]
fn water_neighbor_checks_stop_at_the_first_hot_cell_but_query_both_dimensions() {
    let mut w = World::new();
    let sim = w.sim();
    sim.set_visits(1);
    sim.set_x(2);
    sim.set_z(2);
    edit(|s| {
        s.zone = 14;
        s.values.insert((1, 2, 2), 1);
        s.values.insert((1, 1, 2), 2);
    });
    assert_eq!(w.run(), RESULT);
    let calls = take();
    let start = calls.iter().position(|c| *c == Call::Get(0, 2, 2)).unwrap();
    assert_eq!(
        &calls[start..start + 7],
        [
            Call::Get(0, 2, 2),
            Call::Get(1, 2, 2),
            Call::Get(1, 1, 2),
            Call::Dimension(0),
            Call::Dimension(1),
            Call::Set(1, 2, 2, 1),
            Call::Next
        ]
    );
}

#[test]
fn zero_delta_still_calls_setters_and_exhausted_iterator_resets_before_end_cycle() {
    let mut w = World::new();
    let sim = w.sim();
    sim.set_visits(2);
    sim.set_iterator_state(2);
    *w.constants[8] = 0;
    *w.constants[9] = 0;
    edit(|s| {
        s.source = [100, 100, 0, 0];
        s.zone = 14;
        s.coordinates = [(i32::MAX, i32::MIN), (5, 6)].into();
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!((sim.x(), sim.z()), (10, 12));
    let calls = take();
    assert!(calls.contains(&Call::Set(0, 0, 0, 0)));
    assert!(calls.contains(&Call::Set(1, 0, 0, 0)));
    let resets: Vec<_> = calls.windows(3).filter(|w| w[0] == Call::Reset).collect();
    assert_eq!(
        resets,
        [
            &[Call::Reset, Call::EndCycle, Call::Next][..],
            &[Call::Reset, Call::EndCycle, Call::Next][..]
        ]
    );
    assert!(calls.contains(&Call::Source(0, -2, 0)));
}

#[test]
fn garbage_overflow_caps_the_cell_and_sends_the_remainder_to_supply() {
    let mut w = World::new();
    w.sim().set_visits(1);
    w.common().set_uncollected(32767);
    edit(|s| {
        s.source[2] = 32767;
        s.zones = [1, 14, 14, 14, 1, 14, 14, 14].into();
        s.values.insert((2, 0, 0), 32767);
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!(
        (w.common().uncollected(), w.common().supply(), w.sim().produced()),
        (32767, 32767, 32767)
    );
    let calls = take();
    let cell = calls.iter().position(|c| *c == Call::Set(2, 0, 0, 0)).unwrap();
    assert_eq!(
        &calls[cell..cell + 4],
        [
            Call::Set(2, 0, 0, 0),
            Call::Dispose(65534.0f32.to_bits(), 0, 0),
            Call::Set(2, 0, 0, 32767),
            Call::Supply(32767)
        ]
    );
}

#[test]
fn closed_valve_backflow_limits_to_share_uses_one_for_zero_generation_and_caps_the_cell() {
    for (generated, previous, supply, cell, final_supply, withdrawals) in [
        (25, 0, 100, 50, 75, vec![-25, 0]),
        (25, 0, 10, 35, 0, vec![-10]),
        (0, 5, 100, 6, 99, vec![-1, 0]),
        (20000, 10000, 10000, 32767, 100, vec![-2767, -7133]),
    ] {
        let mut w = World::new();
        w.sim().set_visits(1);
        let common = w.common();
        common.set_capacity(0);
        common.set_supply(supply);
        common.set_uncollected(previous as u32);
        edit(|s| {
            s.source[2] = generated;
            s.zones = [1, 14, 14, 14, 1, 14, 14, 14].into();
            s.values.insert((2, 0, 0), previous);
        });
        assert_eq!(w.run(), RESULT);
        assert_eq!(
            (common.uncollected(), common.supply()),
            (cell as u32, final_supply),
            "generated {generated}"
        );
        let calls = take();
        assert!(calls.contains(&Call::Set(2, 0, 0, cell)));
        let supply_calls: Vec<_> = calls
            .into_iter()
            .filter_map(|c| if let Call::Supply(v) = c { Some(v) } else { None })
            .collect();
        let mut expected = vec![0];
        expected.extend(withdrawals);
        assert_eq!(supply_calls, expected);
    }
}

#[test]
fn uncollected_totals_use_unsigned_comparisons_then_x87_addition() {
    for (initial, previous, expected) in [(u32::MAX, -1, 9), (1, -1, 10), (0x8000_0001, 0, 0x8000_0000)] {
        let mut w = World::new();
        w.sim().set_visits(1);
        w.common().set_uncollected(initial);
        edit(|s| {
            s.source[2] = if initial == 0x8000_0001 { 1 } else { 10 };
            s.zones = [1, 14, 14, 14, 1, 14, 14, 14].into();
            s.values.insert((2, 0, 0), previous);
        });
        assert_eq!(w.run(), RESULT);
        assert_eq!(w.common().uncollected(), expected);
    }
}

#[test]
fn recycling_clamps_to_the_share_and_skips_nonpositive_or_unordered_effects() {
    for (effect, expected_recycled, offered) in [
        (2.0, 100.0, 0.0f32),
        (0.0, 0.0, 100.0),
        (-1.0, 0.0, 100.0),
        (f32::NAN, 0.0, 100.0),
    ] {
        let mut w = World::new();
        w.sim().set_visits(1);
        w.common().set_recycling_effect(effect);
        edit(|s| {
            s.source[2] = 100;
            s.zones = [1, 14, 14, 14, 1, 14, 14, 14].into();
        });
        assert_eq!(w.run(), RESULT);
        assert_eq!(w.sim().recycled(), expected_recycled);
        assert!(take().contains(&Call::Dispose(offered.to_bits(), 0, 0)));
    }
}

#[test]
fn signed_narrowing_of_generated_garbage_keeps_negative_produced_units() {
    let mut w = World::new();
    w.sim().set_visits(1);
    edit(|s| {
        s.source[2] = 32767;
        s.zones = [1, 14, 14, 14, 1, 14, 14, 14].into();
        s.factors.insert((0xE8F7_9C90, 0), 2.0);
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!(w.sim().produced(), -2);
    let calls = take();
    assert!(calls.contains(&Call::Dispose((-2.0f32).to_bits(), 0, 0)));
    assert_eq!(w.common().uncollected(), 0);
}

#[test]
fn valve_flush_offers_the_excess_disposes_it_and_enforces_the_unsigned_limit() {
    let mut w = World::new();
    let common = w.common();
    common.set_capacity(10);
    common.set_supply(1000);
    edit(|s| {
        s.dispose.push_back((0.0, None));
        s.dispose.push_back((250.75, None));
    });
    assert_eq!(w.run(), RESULT);
    assert_eq!(common.supply(), 110);
    let calls = take();
    assert!(calls.contains(&Call::Dispose(990.0f32.to_bits(), 0, 0)));
    assert!(calls.contains(&Call::Supply(-890)));
}

#[test]
fn water_neighbors_cover_edges_and_each_short_circuit_position() {
    for (position, hot, expected_reads) in [
        ((0, 0), None, vec![(1, 0), (0, 1)]),
        ((2, 2), Some((2, 1)), vec![(1, 2), (2, 1)]),
        ((2, 2), Some((3, 2)), vec![(1, 2), (2, 1), (3, 2)]),
        ((2, 2), Some((2, 3)), vec![(1, 2), (2, 1), (3, 2), (2, 3)]),
        ((7, 7), None, vec![(6, 7), (7, 6)]),
    ] {
        let mut w = World::new();
        let sim = w.sim();
        sim.set_visits(1);
        sim.set_x(position.0);
        sim.set_z(position.1);
        edit(|s| {
            s.zone = 14;
            s.values.insert((1, position.0, position.1), 1);
            if let Some((x, z)) = hot {
                s.values.insert((1, x, z), 2);
            }
        });
        assert_eq!(w.run(), RESULT);
        let calls = take();
        let reads: Vec<_> = calls
            .iter()
            .filter_map(|c| {
                if let Call::Get(1, x, z) = *c {
                    if (x, z) != position {
                        Some((x, z))
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(reads, expected_reads);
        assert!(calls.contains(&Call::Set(1, position.0, position.1, i32::from(hot.is_some()))));
    }
}

#[test]
fn allocator_is_acquired_at_start_and_again_when_the_final_clear_removes_it() {
    let mut w = World::new();
    *w._allocator_global = core::ptr::null_mut();
    edit(|s| s.cold_after_clear = true);
    assert_eq!(w.run(), RESULT);
    let calls = take();
    let acquire = [
        Call::Framework,
        Call::Acquire(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseAllocator,
    ];
    assert_eq!(&calls[..3], acquire);
    assert_eq!(
        &calls[calls.len() - 5..],
        [
            Call::Clear,
            acquire[0].clone(),
            acquire[1].clone(),
            acquire[2].clone(),
            Call::Free
        ]
    );
}

fn environment() -> (u16, u16, u32, u32) {
    let mut mxcsr = 0u32;
    let flags: u32;
    // SAFETY: reading the floating environment and direction flag does not alter them.
    unsafe {
        core::arch::asm!("stmxcsr [{mxcsr}]", "pushfd", "pop {flags}",
            mxcsr = in(reg) &mut mxcsr, flags = out(reg) flags);
    }
    (
        crate::x87::control_word(),
        crate::x87::status_word() & 0x3800,
        mxcsr,
        flags & 0x400,
    )
}

#[test]
fn daily_float_operations_preserve_the_environment_and_a_value_below_the_working_stack() {
    let mut w = World::new();
    w.sim().set_visits(1);
    w.common().set_recycling_effect(0.25);
    edit(|s| {
        s.source = [100, 200, 400, 2];
        s.zone = 1;
        s.traffic_value = 8;
    });
    let sentinel = 1.234_567_890_123_456_7f64;
    // SAFETY: the sentinel occupies one register below the routine's balanced working stack.
    unsafe {
        core::arch::asm!("fld qword ptr [{value}]", value = in(reg) &sentinel, out("st(0)") _);
    }
    let before = environment();
    assert_eq!(w.run(), RESULT);
    let after = environment();
    let mut restored = 0.0f64;
    // SAFETY: remove the sentinel after recording the final environment.
    unsafe {
        core::arch::asm!("fstp qword ptr [{value}]", value = in(reg) &mut restored, out("st(0)") _);
    }
    assert_eq!(after, before);
    assert_eq!(restored.to_bits(), sentinel.to_bits());
}
