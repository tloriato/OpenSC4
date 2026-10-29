// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use crate::traffic::test_support::{bytes, float_environment, object, table};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Find(u32, i32, i32),
    Record(usize, i32, i32),
    Other(usize),
    Holder(usize),
    Float(usize, u32, u32),
    City(usize),
    Established(usize),
    Position(usize),
    Get(usize, u32),
    Next(usize),
    Allocate(u32),
    Construct(usize),
    Retain(usize),
    Set(usize, u32, i32),
    Insert(usize, u32),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    connection: usize,
    record: usize,
    other: usize,
    city: usize,
    established: u32,
    city_x: i32,
    city_z: i32,
    cost: Option<u32>,
    properties: Vec<(usize, u32, i32, u32)>,
    next: usize,
    allocation: usize,
    slot: usize,
    position_outputs: [usize; 2],
    change_position_during_property: bool,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
extern "thiscall" fn find(_: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
    log(Call::Find(kind, x, z));
    STATE.with(|s| s.borrow().connection as *mut u8)
}
extern "thiscall" fn record(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    log(Call::Record(this as usize, x, z));
    STATE.with(|s| s.borrow().record as *mut u8)
}
extern "thiscall" fn other(_: *mut u8, conn: *mut u8) -> *mut u8 {
    log(Call::Other(conn as usize));
    STATE.with(|s| s.borrow().other as *mut u8)
}
extern "thiscall" fn holder(this: *mut u8) -> *mut u8 {
    log(Call::Holder(this as usize));
    0x1234 as *mut u8
}
unsafe extern "C" fn float(holder: *mut u8, id: u32, out: *mut u32) -> u32 {
    log(Call::Float(holder as usize, id, unsafe { out.read() }));
    STATE.with(|s| {
        if let Some(bits) = s.borrow().cost {
            unsafe {
                out.write(bits);
            }
        }
    });
    0xABCD_EF01
}
extern "thiscall" fn city(this: *mut u8) -> *mut u8 {
    log(Call::City(this as usize));
    STATE.with(|s| s.borrow().city as *mut u8)
}
extern "thiscall" fn established(this: *mut u8) -> u32 {
    log(Call::Established(this as usize));
    STATE.with(|s| s.borrow().established)
}
unsafe extern "thiscall" fn position(this: *mut u8, x: *mut u32, z: *mut i32) -> u32 {
    log(Call::Position(this as usize));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.position_outputs = [x as usize, z as usize];
        unsafe {
            x.write(s.city_x as u32);
            z.write(s.city_z);
        }
    });
    0x1020_3040
}
unsafe extern "C" fn get(holder: *mut u8, id: u32, out: *mut i32) -> u32 {
    log(Call::Get(holder as usize, id));
    STATE.with(|s| {
        let s = s.borrow();
        if s.change_position_during_property {
            unsafe {
                (s.position_outputs[0] as *mut u32).write(11);
                (s.position_outputs[1] as *mut i32).write(21);
            }
        }
        if let Some(&(_, _, value, result)) = s
            .properties
            .iter()
            .find(|&&(p, k, _, _)| p == holder as usize && k == id)
        {
            unsafe {
                out.write(value);
            }
            result
        } else {
            0xAAAA_BB00
        }
    })
}
unsafe extern "thiscall" fn next(iter: *mut *mut u8) -> u32 {
    log(Call::Next(unsafe { iter.read() } as usize));
    unsafe {
        iter.write(STATE.with(|s| s.borrow().next as *mut u8));
    }
    0
}
extern "C" fn allocate(size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| s.borrow().allocation as *mut u8)
}
extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
    log(Call::Construct(this as usize));
    this
}
extern "thiscall" fn retain(this: *mut u8) -> u32 {
    log(Call::Retain(this as usize));
    2
}
extern "C" fn set(holder: *mut u8, id: u32, value: i32) -> u32 {
    log(Call::Set(holder as usize, id, value));
    0
}
unsafe extern "thiscall" fn insert(this: *mut u8, key: *const u32) -> *mut *mut u8 {
    log(Call::Insert(this as usize, unsafe { key.read() }));
    STATE.with(|s| s.borrow().slot as *mut *mut u8)
}

struct World {
    sim: Object,
    record: Object,
    manager: Object,
    connection: Object,
    other: Object,
    city: Object,
    header: Object,
    holder: Object,
    slot: Box<*mut u8>,
    _tables: Vec<Vec<usize>>,
    _global: Box<*mut u8>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        let tables = vec![
            table(&[(0x2C, find as *const () as usize), (0x34, other as *const () as usize)]),
            table(&[(0x24, city as *const () as usize)]),
            table(&[(0x0C, holder as *const () as usize)]),
            table(&[
                (0xAC, established as *const () as usize),
                (0x14, position as *const () as usize),
            ]),
            table(&[(4, retain as *const () as usize)]),
        ];
        let mut w = Self {
            sim: object(0xD00, &[]),
            record: Object::new(8),
            manager: object(4, &tables[0]),
            connection: object(4, &tables[1]),
            other: object(4, &tables[2]),
            city: object(4, &tables[3]),
            holder: object(0x14, &tables[4]),
            header: Object::new(0x18),
            slot: Box::new(core::ptr::null_mut()),
            _global: Box::new(core::ptr::null_mut()),
            _tables: tables,
            _fakes: vec![
                fake(0x0070_F970, record as *const () as usize),
                fake(0x005F_D4F0, float as *const () as usize),
                fake(0x005F_D450, get as *const () as usize),
                fake(0x005D_17C0, next as *const () as usize),
                fake(0x005E_55E0, allocate as *const () as usize),
                fake(0x005F_F100, construct as *const () as usize),
                fake(0x005F_E330, set as *const () as usize),
                fake(0x0071_4560, insert as *const () as usize),
            ],
        };
        *w._global = w.manager.ptr();
        w._fakes
            .push(fake_data(0x00B4_3D8C, (&mut *w._global as *mut *mut u8).cast()));
        let sim = unsafe { NeighborSim::from_ptr(w.sim.ptr()) };
        sim.set_pools(w.header.ptr());
        sim.set_commute_limit(60.0);
        let head = unsafe { PoolNode::from_ptr(w.header.ptr()) };
        head.set_left(w.header.ptr());
        STATE.with(|s| {
            *s.borrow_mut() = State {
                connection: w.connection.ptr() as usize,
                record: w.record.ptr() as usize,
                city: w.city.ptr() as usize,
                established: 0x1234_0001,
                city_x: 10,
                city_z: 20,
                next: w.header.ptr() as usize,
                allocation: w.holder.ptr() as usize,
                slot: (&mut *w.slot as *mut *mut u8) as usize,
                ..State::default()
            }
        });
        w
    }
    fn run(&mut self, kind: u32) -> (u32, [u32; 3]) {
        let mut frame = [
            fc4p_007153c0 as *const () as u32,
            self.sim.ptr() as u32,
            kind,
            u32::MAX,
            5,
            0,
            0,
            0,
            0,
        ];
        unsafe {
            core::arch::asm!(
            "push [edi + 16]", "push [edi + 12]", "push [edi + 8]", "mov ecx, [edi + 4]",
            "call dword ptr [edi]", "mov [edi + 20], eax",
            "mov eax, [esp - 12]", "mov [edi + 24], eax", "mov eax, [esp - 8]", "mov [edi + 28], eax",
            "mov eax, [esp - 4]", "mov [edi + 32], eax", in("edi") frame.as_mut_ptr(), clobber_abi("C"));
        }
        (frame[5], [frame[6], frame[7], frame[8]])
    }
    fn rec(&mut self) -> ConnectionRecord {
        unsafe { ConnectionRecord::from_ptr(self.record.ptr()) }
    }
    fn prefix(&mut self, kind: u32, has_other: bool) -> Vec<Call> {
        let mut calls = vec![
            Call::Find(kind, -1, 5),
            Call::Record(self.sim.ptr() as usize, -1, 5),
            Call::Other(self.connection.ptr() as usize),
        ];
        if has_other {
            calls.extend([
                Call::Holder(self.other.ptr() as usize),
                Call::Float(0x1234, 0xCA94_7077, 60.0f32.to_bits()),
            ]);
        }
        calls.extend([
            Call::City(self.connection.ptr() as usize),
            Call::Established(self.city.ptr() as usize),
            Call::Position(self.city.ptr() as usize),
        ]);
        calls
    }
    fn pool(&mut self, key: u16, holder: usize, x: Option<i32>, z: Option<i32>) -> Object {
        let mut node = Object::new(0x18);
        let n = unsafe { PoolNode::from_ptr(node.ptr()) };
        n.set_key(key);
        n.set_holder(holder as *mut u8);
        let header = unsafe { PoolNode::from_ptr(self.header.ptr()) };
        header.set_left(node.ptr());
        header.set_parent(node.ptr());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if let Some(x) = x {
                s.properties.push((holder, 0x4A23_576B, x, 0xAABB_CC01));
            }
            if let Some(z) = z {
                s.properties.push((holder, 0x4A23_576C, z, 0xDDEE_FF01));
            }
        });
        node
    }
}

#[test]
fn adding_connection_creates_a_position_pool_and_updates_only_the_record() {
    let _fpu = GameFpu::new();
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.other = w.other.ptr() as usize;
        s.cost = Some(120.0f32.to_bits());
    });
    let sim_before = unsafe { bytes(w.sim.ptr(), 0xD00) };
    let (eax, args) = w.run(2);
    assert_eq!(eax as usize, (&mut *w.slot as *mut *mut u8) as usize);
    assert_eq!(args, [10, 120.0f32.to_bits(), 1]);
    assert_eq!(
        (w.rec().mask(), w.rec().pool(), w.rec().cost_bits()),
        (4, 1, 120.0f32.to_bits())
    );
    assert_eq!(*w.slot, w.holder.ptr());
    assert_eq!(unsafe { bytes(w.sim.ptr(), 0xD00) }, sim_before);
    let mut expected = w.prefix(2, true);
    let p = w.holder.ptr() as usize;
    expected.extend([
        Call::Allocate(0x14),
        Call::Construct(p),
        Call::Retain(p),
        Call::Set(p, 0x4A23_576B, 10),
        Call::Set(p, 0x4A23_576C, 20),
        Call::Insert(w.sim.ptr() as usize + 0xB8, 1),
    ]);
    assert_eq!(calls(), expected);
}

#[test]
fn adding_another_type_reuses_the_pool_and_keeps_the_larger_cost() {
    let mut w = World::new();
    let mut node = w.pool(7, 0x1111, Some(10), Some(20));
    w.rec().set_cost_bits(120.0f32.to_bits());
    w.rec().set_mask(4);
    let env = float_environment();
    let (eax, args) = w.run(1);
    assert_eq!(eax, 0xDDEE_0007);
    assert_eq!(args, [10, 60.0f32.to_bits(), node.ptr() as u32]);
    assert_eq!(
        (w.rec().mask(), w.rec().pool(), w.rec().cost_bits()),
        (6, 7, 120.0f32.to_bits())
    );
    let mut expected = w.prefix(1, false);
    expected.extend([Call::Get(0x1111, 0x4A23_576B), Call::Get(0x1111, 0x4A23_576C)]);
    assert_eq!(calls(), expected);
    assert_eq!(float_environment(), env);
}

#[test]
fn failed_or_mismatched_position_properties_advance_then_choose_smallest_free_key() {
    for (x, z) in [
        (None, Some(20)),
        (Some(10), None),
        (Some(11), Some(20)),
        (Some(10), Some(21)),
    ] {
        let mut w = World::new();
        let mut node = w.pool(1, 0x1111, x, z);
        let (_, args) = w.run(2);
        assert_eq!(w.rec().pool(), 2);
        assert_eq!(args[2], 2);
        let mut expected = w.prefix(2, false);
        expected.push(Call::Get(0x1111, 0x4A23_576B));
        if x.is_some() {
            expected.push(Call::Get(0x1111, 0x4A23_576C));
        }
        expected.push(Call::Next(node.ptr() as usize));
        let p = w.holder.ptr() as usize;
        expected.extend([
            Call::Allocate(0x14),
            Call::Construct(p),
            Call::Retain(p),
            Call::Set(p, 0x4A23_576B, 10),
            Call::Set(p, 0x4A23_576C, 20),
            Call::Insert(w.sim.ptr() as usize + 0xB8, 2),
        ]);
        assert_eq!(calls(), expected);
    }
}

#[test]
fn missing_connection_or_border_record_has_no_writes() {
    for missing_connection in [true, false] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            if missing_connection {
                s.connection = 0;
            } else {
                s.record = 0;
            }
        });
        let before = unsafe { bytes(w.record.ptr(), 8) };
        assert_eq!(w.run(2), (0, [2, u32::MAX, 5]));
        assert_eq!(unsafe { bytes(w.record.ptr(), 8) }, before);
        let mut expected = vec![Call::Find(2, -1, 5)];
        if !missing_connection {
            expected.push(Call::Record(w.sim.ptr() as usize, -1, 5));
        }
        assert_eq!(calls(), expected);
    }
}

#[test]
fn unestablished_or_absent_city_clears_pool_and_retains_return_bytes() {
    for absent in [true, false] {
        let mut w = World::new();
        w.rec().set_pool(42);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.established = 0x1234_5600;
            if absent {
                s.city = 0;
            }
        });
        assert_eq!(
            w.run(2),
            (if absent { 0 } else { 0x1234_5600 }, [2, 60.0f32.to_bits(), 5])
        );
        assert_eq!(w.rec().pool(), 0);
        let mut expected = w.prefix(2, false);
        expected.pop();
        if absent {
            expected.pop();
        }
        assert_eq!(calls(), expected);
    }
}

#[test]
fn cost_comparison_keeps_equal_zero_and_unordered_values_and_handles_shift_width() {
    let _fpu = GameFpu::new();
    for (old, candidate, expected) in [
        (0x8000_0000, 0, 0x8000_0000),
        (0x7FC0_1234, 0x3F80_0000, 0x7FC0_1234),
        (0x3F80_0000, 0x7F80_1234, 0x3F80_0000),
        (0xFF80_0000, 0x7F80_0000, 0x7F80_0000),
    ] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.other = w.other.ptr() as usize;
            s.cost = Some(candidate);
            s.city = 0;
        });
        w.rec().set_cost_bits(old);
        w.run(34);
        assert_eq!(w.rec().cost_bits(), expected);
        assert_eq!(w.rec().mask(), 4);
    }
    let mut w = World::new();
    STATE.with(|s| s.borrow_mut().city = 0);
    w.run(16);
    assert_eq!(w.rec().mask(), 0);
}

#[test]
fn pool_matching_reads_city_position_again_after_property_callbacks() {
    let mut w = World::new();
    let mut node = w.pool(7, 0x1111, Some(11), Some(21));
    STATE.with(|s| s.borrow_mut().change_position_during_property = true);
    assert_eq!(w.run(2), (0xDDEE_0007, [11, 60.0f32.to_bits(), node.ptr() as u32]));
    assert_eq!(w.rec().pool(), 7);
}
