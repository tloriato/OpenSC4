// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Service-building and health-affecting vectors moved from the core residential tests.

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, Clone, PartialEq)]
enum Call {
    Occupant,
    Holder,
    Type,
    Purpose,
    Item(u32),
    Department(u32),
    Change(u32, u32),
    Group(u32),
    Lookup(u32),
    Create(u32, u32),
    Display(u32, u32),
    Funding(bool, u32),
    Count,
    SetCount(u64),
    RemoveLine(u32),
    Register(u32),
    ReadUInt(u32),
    WriteUInt(u32, u32),
    ReadFloat(u32),
    WriteFloat(u32, f32),
    AddRef(usize),
    Release(usize),
    Allocate(usize),
    Erase(usize),
}

#[derive(Clone, Copy)]
struct BudgetItem {
    department: u32,
    line: u32,
    index: u32,
}

struct State {
    calls: Vec<Call>,
    sim: usize,
    objects: [usize; 6],
    items: Vec<(u32, BudgetItem)>,
    groups: Vec<u32>,
    null_department: Option<u32>,
    line_exists: bool,
    count: u64,
    capacity: Option<u32>,
    floats: [Option<f32>; 2],
    allocated: Vec<usize>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            calls: Vec::new(),
            sim: 0,
            objects: [0; 6],
            items: Vec::new(),
            groups: Vec::new(),
            null_department: None,
            line_exists: true,
            count: 0,
            capacity: Some(1500),
            floats: [None; 2],
            allocated: Vec::new(),
        }
    }
}

thread_local! {static STATE:RefCell<State>=RefCell::new(State::default());}
fn state(f: impl FnOnce(&mut State)) {
    STATE.with(|state| f(&mut state.borrow_mut()));
}
fn log(call: Call) {
    state(|state| state.calls.push(call));
}
fn take() -> Vec<Call> {
    STATE.with(|state| std::mem::take(&mut state.borrow_mut().calls))
}

#[repr(C)]
struct Obj {
    table: *const usize,
    id: usize,
    word: u32,
    refs: i32,
}

unsafe fn id(this: *mut Obj) -> usize {
    unsafe { (*this).id }
}
fn object(index: usize) -> *mut u8 {
    STATE.with(|state| state.borrow().objects[index] as *mut u8)
}

unsafe extern "thiscall" fn occupant(this: *mut Obj) -> *mut u8 {
    assert_eq!(unsafe { id(this) }, 0);
    log(Call::Occupant);
    object(1)
}
unsafe extern "thiscall" fn holder(this: *mut Obj) -> *mut u8 {
    assert_eq!(unsafe { id(this) }, 1);
    log(Call::Holder);
    object(2)
}
unsafe extern "thiscall" fn building_type(this: *mut Obj) -> u32 {
    assert_eq!(unsafe { id(this) }, 0);
    log(Call::Type);
    0xDEAD_BEEF
}
unsafe extern "thiscall" fn purpose(this: *mut Obj) -> *const u32 {
    assert_eq!(unsafe { id(this) }, 0);
    log(Call::Purpose);
    unsafe { &raw const (*this).word }
}
unsafe extern "thiscall" fn group(this: *mut Obj, group: u32) -> u32 {
    assert_eq!(unsafe { id(this) }, 1);
    log(Call::Group(group));
    STATE.with(|state| {
        if state.borrow().groups.contains(&group) {
            0xABCD_EF01
        } else {
            0xABCD_EF00
        }
    })
}
unsafe extern "thiscall" fn add_ref(this: *mut Obj) {
    log(Call::AddRef(unsafe { id(this) }));
    unsafe {
        (*this).refs += 1;
    }
}
unsafe extern "thiscall" fn release(this: *mut Obj) {
    log(Call::Release(unsafe { id(this) }));
    unsafe {
        (*this).refs -= 1;
    }
}

unsafe extern "thiscall" fn get_item(this: *mut Obj, holder: *mut u8, purpose: u32, out: *mut u8) -> u32 {
    assert_eq!(unsafe { id(this) }, 5);
    assert_eq!(holder, object(2));
    assert_eq!(unsafe { Item::from_ptr(out).index() }, 0);
    log(Call::Item(purpose));
    STATE.with(|state| {
        if let Some((_, item)) = state.borrow().items.iter().find(|(key, _)| *key == purpose) {
            unsafe {
                Item::from_ptr(out).set_department(item.department);
                Item::from_ptr(out).set_line(item.line);
                Item::from_ptr(out).set_index(item.index);
            }
            0x1234_5601
        } else {
            0x1234_5600
        }
    })
}
unsafe extern "thiscall" fn department(this: *mut Obj, department: u32) -> *mut u8 {
    assert_eq!(unsafe { id(this) }, 5);
    log(Call::Department(department));
    STATE.with(|state| {
        if state.borrow().null_department == Some(department) {
            core::ptr::null_mut()
        } else {
            object(3)
        }
    })
}
unsafe extern "thiscall" fn change(this: *mut Obj, holder: *mut u8, index: u32, line: u32) {
    assert_eq!(unsafe { id(this) }, 5);
    assert_eq!(holder, object(2));
    log(Call::Change(index, line));
}
unsafe extern "thiscall" fn lookup(this: *mut Obj, line: u32) -> *mut u8 {
    assert_eq!(unsafe { id(this) }, 3);
    log(Call::Lookup(line));
    STATE.with(|state| {
        if state.borrow().line_exists {
            object(4)
        } else {
            core::ptr::null_mut()
        }
    })
}
unsafe extern "thiscall" fn create(this: *mut Obj, line: u32, local: u32) -> *mut u8 {
    assert_eq!(unsafe { id(this) }, 3);
    log(Call::Create(line, local));
    state(|state| state.line_exists = true);
    object(4)
}
unsafe extern "thiscall" fn display(this: *mut Obj, flag: u32, value: u32) {
    assert_eq!(unsafe { id(this) }, 4);
    log(Call::Display(flag, value));
}
unsafe extern "thiscall" fn fund(this: *mut Obj, holder: *mut u8, purpose: u32) {
    assert_eq!(unsafe { id(this) }, 3);
    assert_eq!(holder, object(2));
    log(Call::Funding(true, purpose));
}
unsafe extern "thiscall" fn unfund(this: *mut Obj, holder: *mut u8, purpose: u32) {
    assert_eq!(unsafe { id(this) }, 3);
    assert_eq!(holder, object(2));
    log(Call::Funding(false, purpose));
}
unsafe extern "thiscall" fn count(this: *mut Obj) -> u64 {
    assert_eq!(unsafe { id(this) }, 4);
    log(Call::Count);
    STATE.with(|state| state.borrow().count)
}
unsafe extern "thiscall" fn set_count(this: *mut Obj, low: u32, high: u32) {
    assert_eq!(unsafe { id(this) }, 4);
    let value = (u64::from(high) << 32) | u64::from(low);
    log(Call::SetCount(value));
    state(|state| state.count = value);
}
unsafe extern "thiscall" fn remove_line(this: *mut Obj, line: u32) {
    assert_eq!(unsafe { id(this) }, 3);
    log(Call::RemoveLine(line));
}
unsafe extern "thiscall" fn register(this: *mut Obj, id: u32, a: u32, b: u32, c: u32) {
    assert_eq!(unsafe { self::id(this) }, 2);
    assert_eq!([a, b, c], [0, 1, 0]);
    log(Call::Register(id));
}
extern "C" fn read_uint(holder: *mut u8, id: u32, out: *mut u32) -> u32 {
    assert_eq!(holder, object(2));
    log(Call::ReadUInt(id));
    STATE.with(|state| {
        if let Some(value) = state.borrow().capacity {
            unsafe { out.write(value) };
            1
        } else {
            0
        }
    })
}
extern "C" fn write_uint(holder: *mut u8, id: u32, value: u32) {
    assert_eq!(holder, object(2));
    log(Call::WriteUInt(id, value));
}
extern "C" fn read_float(holder: *mut u8, id: u32, out: *mut f32) -> u32 {
    assert_eq!(holder, object(2));
    log(Call::ReadFloat(id));
    let index = if id == 0x6A19_F6B5 {
        0
    } else {
        assert_eq!(id, 0xAA19_F6EA);
        1
    };
    STATE.with(|state| {
        if let Some(value) = state.borrow().floats[index] {
            unsafe { out.write(value) };
            1
        } else {
            0
        }
    })
}
extern "C" fn write_float(holder: *mut u8, id: u32, value: f32) {
    assert_eq!(holder, object(2));
    log(Call::WriteFloat(id, value));
}
extern "thiscall" fn allocate(this: *mut u8, count: u32, tag: u32) -> *mut u8 {
    assert_eq!((count, tag), (1, 0));
    let pointer = Box::into_raw(Box::new([0u32; 3])).cast::<u8>();
    state(|state| {
        state.calls.push(Call::Allocate(this as usize - state.sim));
        state.allocated.push(pointer as usize);
    });
    pointer
}
unsafe extern "thiscall" fn erase(this: *mut u8, out: *mut u32, node: *mut u8) {
    state(|state| state.calls.push(Call::Erase((this as usize).wrapping_sub(state.sim))));
    unsafe {
        let node_view = Node::from_ptr(node);
        let next = node_view.next();
        let previous = node_view.previous();
        Node::from_ptr(next).set_previous(previous);
        Node::from_ptr(previous).set_next(next);
        out.write(next as u32);
        state(|state| state.allocated.retain(|&address| address != node as usize));
        drop(Box::from_raw(node.cast::<[u32; 3]>()));
    }
}

// Every box is referenced by a game-layout pointer before the owning vectors are assembled.
#[allow(clippy::vec_box)]
struct World {
    sim: Object,
    objects: Vec<Box<Obj>>,
    _tables: Vec<Box<[usize; 64]>>,
    heads: Vec<Box<[u32; 3]>>,
    arrays: Vec<Box<[usize; 2]>>,
    _zero: Box<f32>,
    _guards: Vec<Fake>,
    _fpu: GameFpu,
}

impl World {
    fn new() -> Self {
        state(|state| *state = State::default());
        let mut sim = Object::new(0x3D0);
        let mut tables = (0..6).map(|_| Box::new([0usize; 64])).collect::<Vec<_>>();
        for (object, offset, function) in [
            (0, 4, add_ref as *const ()),
            (0, 0x0C, occupant as *const ()),
            (0, 0x10, building_type as *const ()),
            (0, 0x2C, purpose as *const ()),
            (1, 4, add_ref as *const ()),
            (1, 8, release as *const ()),
            (1, 0x18, holder as *const ()),
            (1, 0x58, group as *const ()),
            (2, 0x28, register as *const ()),
            (3, 0x48, create as *const ()),
            (3, 0x4C, remove_line as *const ()),
            (3, 0x50, lookup as *const ()),
            (3, 0x58, fund as *const ()),
            (3, 0x5C, unfund as *const ()),
            (4, 0x20, count as *const ()),
            (4, 0x24, set_count as *const ()),
            (4, 0x48, display as *const ()),
            (5, 0x70, department as *const ()),
            (5, 0xCC, get_item as *const ()),
            (5, 0xD0, change as *const ()),
        ] {
            tables[object][offset / 4] = function as usize;
        }
        let objects = tables
            .iter()
            .enumerate()
            .map(|(id, table)| {
                Box::new(Obj {
                    table: table.as_ptr(),
                    id,
                    word: 0,
                    refs: 0,
                })
            })
            .collect::<Vec<_>>();
        state(|state| {
            state.sim = sim.ptr() as usize;
            state.objects = std::array::from_fn(|i| (&*objects[i] as *const Obj) as usize);
        });
        unsafe {
            BuildingSim::from_ptr(sim.ptr()).set_budget(object(5));
        }
        let mut heads = (0..8).map(|_| Box::new([0u32; 3])).collect::<Vec<_>>();
        for head in &mut heads {
            let pointer = head.as_mut_ptr() as u32;
            head[0] = pointer;
            head[1] = pointer;
        }
        for (offset, index) in [(0x19C, 0), (0x1A0, 1), (0x1A4, 2), (0x1C0, 3)] {
            unsafe {
                sim.ptr()
                    .add(offset)
                    .cast::<u32>()
                    .write(heads[index].as_mut_ptr() as u32);
            }
        }
        let mut arrays = vec![
            Box::new([heads[4].as_mut_ptr() as usize, heads[5].as_mut_ptr() as usize]),
            Box::new([heads[6].as_mut_ptr() as usize, heads[7].as_mut_ptr() as usize]),
        ];
        for (offset, index) in [(0x1A8, 0), (0x1B4, 1)] {
            unsafe {
                sim.ptr()
                    .add(offset)
                    .cast::<*mut usize>()
                    .write(arrays[index].as_mut_ptr());
                sim.ptr()
                    .add(offset + 4)
                    .cast::<*mut usize>()
                    .write(arrays[index].as_mut_ptr().add(2));
            }
        }
        let mut zero = Box::new(0.0f32);
        let guards = vec![
            fake(0x0041_3DD0, allocate as *const () as usize),
            fake(0x0041_3F70, erase as *const () as usize),
            fake(0x005F_D480, read_uint as *const () as usize),
            fake(0x005F_D4F0, read_float as *const () as usize),
            fake(0x005F_E3C0, write_uint as *const () as usize),
            fake(0x005F_E4F0, write_float as *const () as usize),
            fake_data(0x00A8_1054, (&mut *zero as *mut f32).cast()),
        ];
        Self {
            sim,
            objects,
            _tables: tables,
            heads,
            arrays,
            _zero: zero,
            _guards: guards,
            _fpu: GameFpu::new(),
        }
    }
    fn run(&mut self, inserted: u32) {
        unsafe {
            entry(self.sim.ptr(), object(0), inserted);
        }
    }
    fn dirty(&mut self) -> u8 {
        unsafe { BuildingSim::from_ptr(self.sim.ptr()).dirty() }
    }
    fn clear_dirty(&mut self) {
        unsafe {
            BuildingSim::from_ptr(self.sim.ptr()).set_dirty(0);
        }
    }
    fn values(&mut self, head: usize) -> Vec<usize> {
        let head = self.heads[head].as_mut_ptr().cast::<u8>();
        let mut values = Vec::new();
        unsafe {
            let mut node = Node::from_ptr(head).next();
            while node != head {
                values.push(Node::from_ptr(node).occupant() as usize);
                node = Node::from_ptr(node).next();
            }
        }
        values
    }
    fn push(&mut self, head: usize, occupant: *mut u8) {
        let head = self.heads[head].as_mut_ptr().cast::<u8>();
        let node = Box::into_raw(Box::new([0u32; 3])).cast::<u8>();
        state(|state| state.allocated.push(node as usize));
        unsafe {
            let previous = Node::from_ptr(head).previous();
            let view = Node::from_ptr(node);
            view.set_next(head);
            view.set_previous(previous);
            view.set_occupant(occupant);
            Node::from_ptr(previous).set_next(node);
            Node::from_ptr(head).set_previous(node);
        }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        state(|state| {
            for pointer in state.allocated.drain(..) {
                unsafe {
                    drop(Box::from_raw(pointer as *mut [u32; 3]));
                }
            }
        });
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn entry(_this: *mut u8, _building: *mut u8, _inserted: u32) {
    core::arch::naked_asm!("mov dword ptr [esp-0x6c],0x12345678","jmp {entry}",entry=sym fc4p_006fb120);
}

fn ignored_hospital() -> Vec<Call> {
    vec![
        Call::Item(HOSPITAL.staff),
        Call::Occupant,
        Call::Group(0x1507),
        Call::Occupant,
        Call::Group(0x151C),
    ]
}
fn ignored_school() -> Vec<Call> {
    vec![
        Call::Item(SCHOOL.staff),
        Call::Occupant,
        Call::Group(0x1503),
        Call::Occupant,
        Call::Group(0x151B),
    ]
}

#[test]
fn group_recognized_school_initializes_properties_and_appends_in_order() {
    let mut world = World::new();
    state(|state| state.groups.push(0x1503));
    world.run(0xFFFF_0080);
    assert_eq!(world.dirty(), 1);
    assert_eq!(world.values(0), [object(1) as usize]);
    assert_eq!((world.objects[0].refs, world.objects[1].refs), (1, 0));
    let mut expected = vec![Call::Occupant, Call::Holder];
    expected.extend(ignored_hospital());
    expected.extend([
        Call::Item(SCHOOL.staff),
        Call::Occupant,
        Call::Group(0x1503),
        Call::Register(SCHOOL.served),
        Call::WriteUInt(SCHOOL.served, u32::MAX),
        Call::Register(SCHOOL.grade),
        Call::WriteFloat(SCHOOL.grade, 1.0),
        Call::Register(SCHOOL.ratio),
        Call::WriteFloat(SCHOOL.ratio, 1.0),
        Call::ReadUInt(SCHOOL.capacity),
        Call::Register(SCHOOL.effective),
        Call::WriteUInt(SCHOOL.effective, 1500),
        Call::AddRef(0),
        Call::Occupant,
        Call::Allocate(0x19C),
        Call::ReadFloat(0x6A19_F6B5),
        Call::Purpose,
    ]);
    assert_eq!(take(), expected);
}

#[test]
fn hospital_and_school_recognition_groups_and_optional_capacity_leftovers() {
    for (group, head, service) in [
        (0x1507, 1, HOSPITAL),
        (0x151C, 1, HOSPITAL),
        (0x1503, 0, SCHOOL),
        (0x151B, 0, SCHOOL),
    ] {
        let mut world = World::new();
        state(|state| {
            state.groups.push(group);
            state.capacity = None;
        });
        world.run(1);
        assert_eq!(world.values(head), [object(1) as usize]);
        assert!(take().contains(&Call::WriteUInt(service.effective, 0x1234_5678)));
    }
}

#[test]
fn a_building_can_enter_both_service_lists_and_the_health_pending_list() {
    let mut world = World::new();
    state(|state| {
        state.groups.extend([0x1507, 0x1503]);
        state.floats = [Some(1.0), Some(1.0)];
    });
    world.run(1);
    for head in 0..3 {
        assert_eq!(world.values(head), [object(1) as usize]);
    }
    assert_eq!((world.objects[0].refs, world.objects[1].refs), (2, 1));
    let calls = take();
    assert!(
        calls.iter().position(|call| *call == Call::Allocate(0x1A0)).unwrap()
            < calls.iter().position(|call| *call == Call::Item(SCHOOL.staff)).unwrap()
    );
    assert!(
        calls.iter().position(|call| *call == Call::Allocate(0x19C)).unwrap()
            < calls.iter().position(|call| *call == Call::Allocate(0x1A4)).unwrap()
    );
}

#[test]
fn staff_and_vehicle_budget_creation_counting_and_registration_follow_call_order() {
    let mut world = World::new();
    state(|state| {
        state.items.extend([
            (
                HOSPITAL.staff,
                BudgetItem {
                    department: 10,
                    line: 0,
                    index: 23,
                },
            ),
            (
                HOSPITAL.vehicle,
                BudgetItem {
                    department: 11,
                    line: 44,
                    index: 55,
                },
            ),
        ]);
        state.line_exists = false;
        state.count = u64::MAX;
        state.capacity = None;
    });
    world.run(1);
    let calls = take();
    assert_eq!(
        &calls[..15],
        &[
            Call::Occupant,
            Call::Holder,
            Call::Item(HOSPITAL.staff),
            Call::Type,
            Call::Change(23, 0xDEAD_BEEF),
            Call::Department(10),
            Call::Lookup(0xDEAD_BEEF),
            Call::Create(0xDEAD_BEEF, 1),
            Call::Display(2, 1),
            Call::Funding(true, HOSPITAL.staff),
            Call::Lookup(0xDEAD_BEEF),
            Call::Count,
            Call::SetCount(0),
            Call::Item(HOSPITAL.vehicle),
            Call::Department(11)
        ]
    );
    assert_eq!(calls[15], Call::Funding(true, HOSPITAL.vehicle));
    assert!(calls.contains(&Call::WriteUInt(HOSPITAL.effective, object(3) as u32)));
    assert!(!calls
        .iter()
        .any(|call| matches!(call, Call::Group(_)) && calls.iter().position(|c| c == call).unwrap() < 16));
    assert_eq!(world.values(1), [object(1) as usize]);
}

#[test]
fn null_staff_or_vehicle_department_aborts_remaining_work() {
    for vehicle in [false, true] {
        let mut world = World::new();
        state(|state| {
            state.items.push((
                HOSPITAL.staff,
                BudgetItem {
                    department: 10,
                    line: 20,
                    index: 30,
                },
            ));
            state.null_department = Some(if vehicle { 11 } else { 10 });
            if vehicle {
                state.items.push((
                    HOSPITAL.vehicle,
                    BudgetItem {
                        department: 11,
                        line: 21,
                        index: 31,
                    },
                ));
            }
        });
        world.run(1);
        assert_eq!(world.dirty(), 0);
        let calls = take();
        assert_eq!(calls.last(), Some(&Call::Department(if vehicle { 11 } else { 10 })));
        assert!(!calls.contains(&Call::Item(SCHOOL.staff)));
        assert!(!calls
            .iter()
            .any(|call| matches!(call, Call::Register(_) | Call::Purpose)));
    }
}

#[test]
fn school_capacity_failure_preserves_line_item_pointer() {
    let mut world = World::new();
    state(|state| {
        state.items.push((
            SCHOOL.staff,
            BudgetItem {
                department: 10,
                line: 20,
                index: 30,
            },
        ));
        state.capacity = None;
    });
    world.run(1);
    assert!(take().contains(&Call::WriteUInt(SCHOOL.effective, object(4) as u32)));
}

#[test]
fn removals_search_categories_before_pending_and_release_only_an_erased_occupant() {
    for (service, group, categorized, pending) in [(HOSPITAL, 0x1507, 7, 1), (SCHOOL, 0x1503, 5, 0)] {
        for head in [categorized, pending] {
            let mut world = World::new();
            state(|state| state.groups.push(group));
            world.push(head, object(1));
            let expected_erase = if head == pending {
                service.pending
            } else {
                (world.arrays[usize::from(!service.school)].as_ptr() as usize + 4)
                    .wrapping_sub(world.sim.ptr() as usize)
            };
            world.run(0xFFFF_FF00);
            assert!(world.values(head).is_empty());
            assert_eq!(world.dirty(), 1);
            let calls = take();
            assert!(calls.contains(&Call::Erase(expected_erase)));
            assert_eq!(calls.iter().filter(|call| **call == Call::Release(1)).count(), 1);
            world.clear_dirty();
            world.run(0);
            assert_eq!(world.dirty(), 0);
            assert!(!take().contains(&Call::Release(1)));
        }
    }
}

#[test]
fn zero_budget_line_count_removes_line_and_underflow_wraps() {
    for (initial, expected, remove) in [(1, 0, true), (0, u64::MAX, false), (0x1_0000_0000, 0xFFFF_FFFF, false)] {
        let mut world = World::new();
        state(|state| {
            state.items.push((
                SCHOOL.staff,
                BudgetItem {
                    department: 10,
                    line: 20,
                    index: 30,
                },
            ));
            state.count = initial;
        });
        world.run(0);
        let calls = take();
        let count = calls.iter().position(|call| *call == Call::SetCount(expected)).unwrap();
        assert_eq!(calls[count + 1], Call::Count);
        assert_eq!(calls.contains(&Call::RemoveLine(20)), remove);
        assert!(calls.contains(&Call::Funding(false, SCHOOL.staff)));
    }
}

#[test]
fn health_affecting_buildings_append_and_remove_with_occupant_references() {
    let mut world = World::new();
    state(|state| state.floats = [Some(1.0), Some(2.0)]);
    world.run(1);
    assert_eq!(world.values(2), [object(1) as usize]);
    assert_eq!(world.objects[1].refs, 1);
    let calls = take();
    assert!(calls.ends_with(&[
        Call::ReadFloat(0x6A19_F6B5),
        Call::ReadFloat(0xAA19_F6EA),
        Call::Occupant,
        Call::AddRef(1),
        Call::Occupant,
        Call::Allocate(0x1A4),
        Call::Purpose
    ]));
    world.clear_dirty();
    world.run(0);
    assert!(world.values(2).is_empty());
    assert_eq!(world.objects[1].refs, 0);
    assert_eq!(world.dirty(), 1);
    assert!(take().contains(&Call::Erase(0x1A4)));
    world.push(3, object(1));
    world.clear_dirty();
    world.run(0);
    assert!(world.values(3).is_empty());
    assert!(take().contains(&Call::Erase(0x1C0)));
    world.clear_dirty();
    world.run(0);
    assert_eq!(world.dirty(), 1);
    assert!(!take().contains(&Call::Release(1)));
}

#[test]
fn health_properties_require_both_success_and_strict_positive_values() {
    for properties in [
        [None, Some(1.0)],
        [Some(1.0), None],
        [Some(0.0), Some(1.0)],
        [Some(-1.0), Some(1.0)],
        [Some(f32::NAN), Some(1.0)],
        [Some(1.0), Some(f32::NAN)],
    ] {
        let mut world = World::new();
        state(|state| state.floats = properties);
        world.run(1);
        assert_eq!(world.dirty(), 0);
        assert!(world.values(2).is_empty());
        let calls = take();
        assert_eq!(calls.contains(&Call::ReadFloat(0xAA19_F6EA)), properties[0].is_some());
    }
}

#[test]
fn final_purpose_test_writes_only_the_dirty_byte_for_other_buildings() {
    let mut world = World::new();
    world.objects[0].word = 1;
    let before = unsafe { std::slice::from_raw_parts(world.sim.ptr(), 0x3D0).to_vec() };
    world.run(1);
    assert_eq!(world.dirty(), 1);
    let mut expected = before;
    expected[0xA0] = 1;
    assert_eq!(unsafe { std::slice::from_raw_parts(world.sim.ptr(), 0x3D0) }, expected);
    let mut calls = vec![Call::Occupant, Call::Holder];
    calls.extend(ignored_hospital());
    calls.extend(ignored_school());
    calls.extend([Call::ReadFloat(0x6A19_F6B5), Call::Purpose]);
    assert_eq!(take(), calls);
}
