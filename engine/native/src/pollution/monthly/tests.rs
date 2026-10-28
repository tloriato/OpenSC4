// SPDX-License-Identifier: GPL-3.0-or-later
//! Monthly expense and history vectors on native objects, including deal-list lifetimes.

// Nodes, interfaces and tables keep stable addresses while their owning vectors grow.
#![allow(clippy::vec_box)]

use super::*;
use crate::testing::{fake, fake_data, Fake, GameFpu, Object};
use std::cell::RefCell;

const RESULT: u64 = 0xFEDC_BA98_7654_3210;
const DIRTY_RESULT: u64 = 0x1234_5678_ABCD_EF90;

#[derive(Clone, Debug, PartialEq)]
enum Call {
    Framework,
    Acquire(u32, u32),
    ReleaseAllocator,
    Allocate(u32),
    Find(u32),
    Used(u32, u32),
    Amount(u32),
    Release(u32),
    Free(u32),
    Clear,
    Item(u32),
    Charge(i64),
    History(u32, u32),
    Age(usize, u32, u32),
    Reset(usize, u32, u32),
    Energy,
    Visuals,
    Dirty,
    Destroy,
}
#[repr(C)]
struct Node {
    next: *mut u8,
    previous: *mut u8,
    occupant: *mut u8,
}
#[repr(C)]
struct Deal {
    table: *const usize,
    amount: i32,
    id: u32,
}

struct State {
    calls: Vec<Call>,
    sim: *mut u8,
    allocator: *mut u8,
    allocator_global: *mut *mut u8,
    framework: *mut u8,
    item: *mut u8,
    deal_table: *const usize,
    imports: Vec<i32>,
    exports: Vec<i32>,
    nodes: Vec<Box<Node>>,
    deals: Vec<Box<Deal>>,
    clear_allocator_on_release: bool,
    mutate_history: bool,
    replace_occupant_on_used: bool,
}
impl Default for State {
    fn default() -> Self {
        Self {
            calls: vec![],
            sim: core::ptr::null_mut(),
            allocator: core::ptr::null_mut(),
            allocator_global: core::ptr::null_mut(),
            framework: core::ptr::null_mut(),
            item: core::ptr::null_mut(),
            deal_table: core::ptr::null(),
            imports: vec![],
            exports: vec![],
            nodes: vec![],
            deals: vec![],
            clear_allocator_on_release: false,
            mutate_history: false,
            replace_occupant_on_used: false,
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

extern "C" fn framework() -> *mut u8 {
    log(Call::Framework);
    STATE.with(|s| s.borrow().framework)
}
extern "thiscall" fn acquire(_this: *mut u8, cls: u32, iid: u32, output: *mut *mut u8) {
    log(Call::Acquire(cls, iid));
    STATE.with(|s| unsafe { output.write(s.borrow().allocator) });
}
extern "thiscall" fn release_allocator(pointer: *mut *mut u8) {
    log(Call::ReleaseAllocator);
    STATE.with(|s| {
        let s = s.borrow();
        assert_eq!(unsafe { pointer.read() }, s.allocator);
        assert_eq!(unsafe { s.allocator_global.read() }, s.allocator);
    });
}
extern "thiscall" fn allocate(this: *mut u8, size: u32) -> *mut u8 {
    log(Call::Allocate(size));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this, s.allocator);
        let mut node = Box::new(Node {
            next: core::ptr::null_mut(),
            previous: core::ptr::null_mut(),
            occupant: 0x1122_3344usize as *mut u8,
        });
        let ptr = (&mut *node as *mut Node).cast();
        s.nodes.push(node);
        ptr
    })
}
extern "thiscall" fn find_deals(_this: *mut u8, id: u32, list: *mut *mut u8) {
    log(Call::Find(id));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let amounts = if id == 0x4773_FFE5 {
            s.imports.clone()
        } else {
            assert_eq!(id, 0x4773_FFE4);
            s.exports.clone()
        };
        let sentinel = unsafe { list.read() };
        assert_eq!(unsafe { ListNode::from_ptr(sentinel).next() }, sentinel);
        for (i, amount) in amounts.into_iter().enumerate() {
            let deal_id = (if id == 0x4773_FFE5 { 100 } else { 200 }) + i as u32;
            let mut deal = Box::new(Deal {
                table: s.deal_table,
                amount,
                id: deal_id,
            });
            let mut node = Box::new(Node {
                next: sentinel,
                previous: unsafe { ListNode::from_ptr(sentinel).previous() },
                occupant: (&mut *deal as *mut Deal).cast(),
            });
            let ptr = (&mut *node as *mut Node).cast();
            unsafe {
                ListNode::from_ptr(node.previous).set_next(ptr);
                ListNode::from_ptr(sentinel).set_previous(ptr);
            }
            s.nodes.push(node);
            s.deals.push(deal);
        }
    });
}
extern "thiscall" fn used(this: *mut Deal, amount: u32) {
    let id = unsafe { (*this).id };
    log(Call::Used(id, amount));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        if s.replace_occupant_on_used {
            let mut replacement = Box::new(Deal {
                table: s.deal_table,
                amount: -999,
                id: id + 1000,
            });
            let ptr = (&mut *replacement as *mut Deal).cast();
            s.nodes.iter_mut().find(|n| n.occupant == this.cast()).unwrap().occupant = ptr;
            s.deals.push(replacement);
        }
    });
}
extern "thiscall" fn amount(this: *mut Deal) -> i32 {
    log(Call::Amount(unsafe { (*this).id }));
    unsafe { (*this).amount }
}
extern "thiscall" fn release(this: *mut Deal) {
    log(Call::Release(unsafe { (*this).id }));
    STATE.with(|s| {
        let s = s.borrow();
        let node = s.nodes.iter().find(|n| n.occupant == this.cast()).unwrap();
        unsafe {
            assert_eq!(ListNode::from_ptr(node.previous).next(), node.next);
            assert_eq!(ListNode::from_ptr(node.next).previous(), node.previous);
            if s.clear_allocator_on_release {
                s.allocator_global.write(core::ptr::null_mut());
            }
        }
    });
}
extern "thiscall" fn free_node(this: *mut u8, pointer: *mut Node) -> u64 {
    STATE.with(|s| assert_eq!(this, s.borrow().allocator));
    log(Call::Free(unsafe { (*(*pointer).occupant.cast::<Deal>()).id }));
    RESULT
}
extern "thiscall" fn clear(list: *mut *mut u8) {
    log(Call::Clear);
    unsafe {
        let sentinel = list.read();
        assert_eq!(ListNode::from_ptr(sentinel).next(), sentinel);
        assert_eq!(ListNode::from_ptr(sentinel).previous(), sentinel);
        assert_eq!(ListNode::from_ptr(sentinel).occupant() as u32, 0x1122_3344);
    }
}
extern "fastcall" fn destroy(list: *mut *mut u8, edx: u32) -> u64 {
    log(Call::Destroy);
    assert_eq!(edx, (DIRTY_RESULT >> 32) as u32);
    unsafe {
        assert_eq!(ListNode::from_ptr(list.read()).next(), list.read());
    }
    RESULT
}
extern "thiscall" fn item(_this: *mut u8, id: u32) -> *mut u8 {
    log(Call::Item(id));
    STATE.with(|s| s.borrow().item)
}
extern "thiscall" fn charge(_this: *mut u8, low: u32, high: u32) {
    log(Call::Charge(registers(low, high) as i64));
}
extern "thiscall" fn history(_this: *mut u8, id: u32, value: u32) {
    log(Call::History(id, value));
    STATE.with(|s| {
        let s = s.borrow();
        if s.mutate_history && id == 0xEA11_0158 {
            unsafe {
                let month = MonthView::from_ptr(s.sim);
                month.set_imported(111);
                month.set_recycled(222.75);
            }
        }
    });
}
extern "thiscall" fn age(this: *mut u8, list: *mut u8, used: u32, capacity: u32) {
    STATE.with(|s| assert_eq!(this, s.borrow().sim));
    log(Call::Age((list as usize).wrapping_sub(this as usize), used, capacity));
    let month = unsafe { MonthView::from_ptr(this) };
    assert_eq!(
        [
            month.produced(),
            month.imported(),
            month.exported(),
            month.incinerated(),
            month.energy(),
            month.landfilled()
        ],
        [0; 6]
    );
    assert_eq!(month.recycled().to_bits(), 0);
    assert_eq!([month.abducted(), month.export_capacity()], [0; 2]);
}
extern "thiscall" fn reset(this: *mut u8, list: *mut u8, used: u32, previous: u32) {
    STATE.with(|s| assert_eq!(this, s.borrow().sim));
    log(Call::Reset((list as usize).wrapping_sub(this as usize), used, previous));
}
extern "thiscall" fn energy(this: *mut u8) {
    STATE.with(|s| assert_eq!(this, s.borrow().sim));
    log(Call::Energy);
}
extern "thiscall" fn visuals(this: *mut u8) {
    STATE.with(|s| assert_eq!(this, s.borrow().sim));
    log(Call::Visuals);
}
extern "thiscall" fn dirty(_this: *mut u8) -> u64 {
    log(Call::Dirty);
    DIRTY_RESULT
}

struct World {
    object: Object,
    allocator_global: Box<*mut u8>,
    _history_global: Box<*mut u8>,
    _neighbors_global: Box<*mut u8>,
    _objects: Vec<Box<usize>>,
    _tables: Vec<Box<[usize; 32]>>,
    _zero: Box<f64>,
    _fakes: Vec<Fake>,
    _fpu: GameFpu,
}
impl World {
    fn new(imports: &[i32], exports: &[i32]) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut tables = vec![];
        let mut objects = vec![];
        for slots in [
            vec![
                (0x0C, allocate as *const () as usize),
                (0x10, free_node as *const () as usize),
            ],
            vec![(0x14, acquire as *const () as usize)],
            vec![
                (0x78, find_deals as *const () as usize),
                (0x7C, dirty as *const () as usize),
            ],
            vec![(0x50, item as *const () as usize)],
            vec![(0x30, charge as *const () as usize)],
            vec![(0x30, history as *const () as usize)],
            vec![
                (0x54, used as *const () as usize),
                (0x28, amount as *const () as usize),
                (0x08, release as *const () as usize),
            ],
        ] {
            let mut table = Box::new([0; 32]);
            for (offset, function) in slots {
                table[offset / 4] = function;
            }
            objects.push(Box::new(table.as_ptr() as usize));
            tables.push(table);
        }
        let pointers: Vec<*mut u8> = objects.iter_mut().map(|o| (&mut **o as *mut usize).cast()).collect();
        let mut allocator_global = Box::new(pointers[0]);
        let mut history_global = Box::new(pointers[5]);
        let mut neighbors_global = Box::new(pointers[2]);
        let mut zero = Box::new(0.0f64);
        let mut fakes: Vec<_> = [
            (0x0087_93EC, framework as *const () as usize),
            (0x0041_1870, release_allocator as *const () as usize),
            (0x0042_1660, clear as *const () as usize),
            (0x0042_1760, destroy as *const () as usize),
            (0x006E_3D00, age as *const () as usize),
            (0x006E_3C80, reset as *const () as usize),
            (0x006E_3F50, energy as *const () as usize),
            (0x006E_63C0, visuals as *const () as usize),
        ]
        .into_iter()
        .map(|(a, f)| fake(a, f))
        .collect();
        fakes.push(fake_data(0x00B2_0D58, (&mut *allocator_global as *mut *mut u8).cast()));
        fakes.push(fake_data(0x00B4_3DC0, (&mut *history_global as *mut *mut u8).cast()));
        fakes.push(fake_data(0x00B4_3D8C, (&mut *neighbors_global as *mut *mut u8).cast()));
        fakes.push(fake_data(0x00A8_0990, (&mut *zero as *mut f64).cast()));
        let mut object = Object::new(0x240);
        let base = object.ptr();
        let month = unsafe { MonthView::from_ptr(base) };
        month.set_department(pointers[3]);
        month.set_neighbors(pointers[2]);
        month.set_cost_per_unit(0.005);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.sim = base;
            s.allocator = pointers[0];
            s.allocator_global = &mut *allocator_global;
            s.framework = pointers[1];
            s.item = pointers[4];
            s.deal_table = tables[6].as_ptr();
            s.imports = imports.to_vec();
            s.exports = exports.to_vec();
        });
        Self {
            object,
            allocator_global,
            _history_global: history_global,
            _neighbors_global: neighbors_global,
            _objects: objects,
            _tables: tables,
            _zero: zero,
            _fakes: fakes,
            _fpu: GameFpu::new(),
        }
    }
    fn month(&mut self) -> MonthView {
        unsafe { MonthView::from_ptr(self.object.ptr()) }
    }
    fn run(&mut self) -> u64 {
        unsafe { fc4p_006e7b60(self.object.ptr()) }
    }
    fn bytes(&mut self) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(self.object.ptr(), 0x240).to_vec() }
    }
}
fn tail() -> Vec<Call> {
    vec![
        Call::Age(0x68, 0x28EC_F510, 0xC8EB_D131),
        Call::Age(0x6C, 0xE8EC_F641, 0xA8EB_CF0C),
        Call::Age(0x74, 0, 0),
        Call::Reset(0x68, 0x28EC_F510, 0x08FC_DB69),
        Call::Reset(0x6C, 0xE8EC_F641, 0xA8FC_DBA8),
        Call::Energy,
        Call::Reset(0x70, 0x88EC_F76C, 0x28FC_DBB0),
        Call::Visuals,
        Call::Dirty,
        Call::Destroy,
    ]
}

#[test]
fn month_resets_usage_books_cost_rolls_counters_and_calls_facilities_in_order() {
    let mut w = World::new(&[800, 900], &[100, -50]);
    let month = w.month();
    for (offset, value) in [
        (0x140, 0x1234_5678_8765_4321i64),
        (0x148, 22),
        (0x150, -33),
        (0x160, 1000),
        (0x168, 4000),
        (0x170, 5000),
    ] {
        unsafe {
            w.object.ptr().add(offset).cast::<i64>().write_unaligned(value);
        }
    }
    month.set_recycled(77.75);
    month.set_export_capacity(99);
    month.set_abducted(u32::MAX);
    let before = w.bytes();
    assert_eq!(w.run(), RESULT);
    let mut expected = before.clone();
    for offset in [0x140, 0x148, 0x150, 0x158, 0x160, 0x168, 0x170] {
        expected[offset..offset + 8].fill(0);
        expected[offset + 0x38..offset + 0x40].copy_from_slice(&before[offset..offset + 8]);
    }
    expected[0x1B0..0x1B8].fill(0);
    expected[0x1B8..0x1BC].copy_from_slice(&u32::MAX.to_le_bytes());
    expected[0x1C8..0x1D0].copy_from_slice(&50i64.to_le_bytes());
    assert_eq!(w.bytes(), expected);
    let mut calls = vec![
        Call::Allocate(12),
        Call::Find(0x4773_FFE5),
        Call::Used(100, 0),
        Call::Release(100),
        Call::Free(100),
        Call::Used(101, 0),
        Call::Release(101),
        Call::Free(101),
        Call::Clear,
        Call::Find(0x4773_FFE4),
        Call::Used(200, 0),
        Call::Amount(200),
        Call::Release(200),
        Call::Free(200),
        Call::Used(201, 0),
        Call::Amount(201),
        Call::Release(201),
        Call::Free(201),
        Call::Item(20),
        Call::Charge(50),
        Call::History(0xEA11_0158, 0x8765_4321),
        Call::History(0x8AB2_F258, 22),
        Call::History(0x6AB2_F260, (-33i32) as u32),
        Call::History(0xEA11_016F, 77),
        Call::History(0xEA11_0197, 1000),
        Call::History(0xEA11_01A1, 4000),
        Call::History(0xEA11_01AA, 5000),
    ];
    calls.extend(tail());
    assert_eq!(take(), calls);
}
#[test]
fn allocator_is_acquired_on_a_cold_start_and_after_a_release_clears_the_global() {
    let mut w = World::new(&[1], &[2]);
    *w.allocator_global = core::ptr::null_mut();
    STATE.with(|s| s.borrow_mut().clear_allocator_on_release = true);
    assert_eq!(w.run(), RESULT);
    let calls = take();
    let acquire = [
        Call::Framework,
        Call::Acquire(0x3AE4_BEA3, 0x3AE4_BEAB),
        Call::ReleaseAllocator,
    ];
    assert_eq!(&calls[..3], acquire);
    for id in [100, 200] {
        let release = calls.iter().position(|c| *c == Call::Release(id)).unwrap();
        assert_eq!(&calls[release + 1..release + 4], acquire);
    }
    assert!(!(*w.allocator_global).is_null());
}
#[test]
fn expense_uses_signed_wrapping_64_bit_units_and_the_games_truncation() {
    let mut w = World::new(&[], &[]);
    for (a, b, c, rate) in [
        (1000, 4000, 0, 0.005f32),
        (i64::MAX, 1, 0, 1.0),
        (i64::MAX - 2, 4, 3, 0.5),
        (-100, 0, 0, 0.019),
        (16_777_217, 0, 0, 1.0),
        (0, 0, 0, f32::NAN),
        (1, 0, 0, f32::INFINITY),
    ] {
        let month = w.month();
        month.set_incinerated(a);
        month.set_energy(b);
        month.set_landfilled(c);
        month.set_cost_per_unit(rate);
        let expected = multiply_float(F80::from_i64(b.wrapping_add(c).wrapping_add(a)), rate).ftol();
        assert_eq!(w.run(), RESULT);
        assert_eq!(month.cost(), expected);
        assert!(take().contains(&Call::Charge(expected)));
    }
}
#[test]
fn callbacks_update_later_histories_and_the_released_node_occupant() {
    let mut w = World::new(&[9], &[-5]);
    let month = w.month();
    month.set_incinerated(100);
    month.set_imported(55);
    month.set_recycled(66.5);
    month.set_cost_per_unit(1.0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.mutate_history = true;
        s.replace_occupant_on_used = true;
    });
    assert_eq!(w.run(), RESULT);
    let calls = take();
    for call in [
        Call::Amount(200),
        Call::Release(1200),
        Call::Free(1200),
        Call::Charge(95),
        Call::History(0x8AB2_F258, 111),
        Call::History(0xEA11_016F, 222),
    ] {
        assert!(calls.contains(&call), "missing {call:?}: {calls:?}");
    }
    unsafe {
        assert_eq!(w.object.ptr().add(0x180).cast::<i64>().read_unaligned(), 55);
        assert_eq!(w.object.ptr().add(0x190).cast::<f64>().read_unaligned(), 66.5);
    }
}
