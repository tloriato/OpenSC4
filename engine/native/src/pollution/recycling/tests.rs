// SPDX-License-Identifier: GPL-3.0-or-later
//! Population weighting cases moved from core pollution, with allocation and list calls recorded.

use super::*;
use crate::testing::{fake, fake_data, GameFpu, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Call {
    Population,
    Allocate,
    Application,
    Acquire(u32, u32),
    Release,
    Capacity(u32),
    Assign(Vec<u32>),
    Holder(u32),
    Served(u32),
    Destroy(Vec<u32>),
}
struct State {
    calls: Vec<Call>,
    // Boxed nodes keep their game addresses throughout the calculation.
    #[allow(clippy::vec_box)]
    allocations: Vec<Box<[u32; 3]>>,
    allocator: *mut u8,
    fail: Option<u32>,
    fail_after_sort: bool,
    sorted: bool,
}
thread_local! {
    static STATE: RefCell<State> = const { RefCell::new(State {
        calls: Vec::new(), allocations: Vec::new(), allocator: core::ptr::null_mut(),
        fail: None, fail_after_sort: false, sorted: false,
    }) };
}
fn log(c: Call) {
    STATE.with(|s| s.borrow_mut().calls.push(c));
}
fn take_calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}
#[repr(C)]
struct Occupant {
    table: *const usize,
    id: u32,
    capacity: f32,
    served: f32,
    has_served: bool,
}
extern "thiscall" fn population(this: *mut u32) -> i32 {
    log(Call::Population);
    unsafe { this.add(1).read() as i32 }
}
extern "thiscall" fn allocate(_this: *mut u8, size: u32) -> *mut u8 {
    assert_eq!(size, 12);
    log(Call::Allocate);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        let mut node = Box::new([0u32; 3]);
        let p = node.as_mut_ptr().cast();
        s.allocations.push(node);
        p
    })
}
extern "thiscall" fn capacity(_this: *mut u8, occupant: *mut Occupant, output: *mut f32) -> u32 {
    let o = unsafe { &*occupant };
    log(Call::Capacity(o.id));
    STATE.with(|s| {
        let s = s.borrow();
        if s.fail == Some(o.id) || (s.sorted && s.fail_after_sort) {
            return 0;
        }
        unsafe { output.write_unaligned(o.capacity) };
        1
    })
}
extern "thiscall" fn holder(this: *mut Occupant) -> *mut u8 {
    log(Call::Holder(unsafe { (*this).id }));
    this.cast()
}
extern "C" fn served(this: *mut Occupant, id: u32, output: *mut f32) -> u32 {
    assert_eq!(id, 0x68ED_1736);
    let o = unsafe { &*this };
    log(Call::Served(o.id));
    if o.has_served {
        unsafe { output.write_unaligned(o.served) };
        1
    } else {
        0
    }
}
unsafe fn list_values(head: *mut u8) -> Vec<*mut Occupant> {
    unsafe {
        let mut result = vec![];
        let mut node = ListNode::from_ptr(head).next();
        while node != head {
            result.push(ListNode::from_ptr(node).occupant().cast());
            node = ListNode::from_ptr(node).next();
        }
        result
    }
}
extern "thiscall" fn assign(destination: *mut *mut u8, source: *mut *mut u8) {
    unsafe {
        let values = list_values(*source);
        log(Call::Assign(values.iter().map(|&p| (*p).id).collect()));
        let head = *destination;
        let mut node = ListNode::from_ptr(head).next();
        let mut previous = head;
        for value in values {
            assert_ne!(node, head);
            ListNode::from_ptr(node).set_occupant(value.cast());
            previous = node;
            node = ListNode::from_ptr(node).next();
        }
        ListNode::from_ptr(previous).set_next(head);
        ListNode::from_ptr(head).set_previous(previous);
    }
    STATE.with(|s| s.borrow_mut().sorted = true);
}
extern "thiscall" fn destroy(head: *mut *mut u8) {
    let ids = unsafe { list_values(*head).iter().map(|&p| (*p).id).collect() };
    log(Call::Destroy(ids));
}
extern "C" fn application() -> *mut u8 {
    log(Call::Application);
    STATE.with(|s| s.borrow().allocator)
}
extern "thiscall" fn acquire(this: *mut u8, class: u32, iid: u32, output: *mut *mut u8) {
    log(Call::Acquire(class, iid));
    unsafe { output.write(this) };
}
extern "thiscall" fn release(reference: *mut *mut u8) {
    log(Call::Release);
    unsafe { reference.write(core::ptr::null_mut()) };
}
fn table(slots: &[(usize, usize)]) -> [usize; 60] {
    let mut table = [0; 60];
    for &(slot, f) in slots {
        table[slot / 4] = f;
    }
    table
}

#[test]
fn recycling_effect_sorts_descending_and_weights_full_served_populations() {
    let _fpu = GameFpu::new();
    run(
        1000,
        &[(1, 0.2, 1000.0, true), (2, 0.5, 600.0, true)],
        None,
        false,
        false,
        2,
        0.3125,
    );
    assert_eq!(
        take_calls(),
        [
            Call::Population,
            Call::Allocate,
            Call::Capacity(1),
            Call::Allocate,
            Call::Capacity(2),
            Call::Capacity(1),
            Call::Allocate,
            Call::Assign(vec![2, 1]),
            Call::Holder(2),
            Call::Served(2),
            Call::Capacity(2),
            Call::Holder(1),
            Call::Served(1),
            Call::Capacity(1),
            Call::Destroy(vec![2, 1]),
        ]
    );
}

#[allow(clippy::too_many_arguments)]
fn run(
    population_value: i32,
    facilities: &[(u32, f32, f32, bool)],
    fail: Option<u32>,
    late_failure: bool,
    lazy: bool,
    count: u32,
    expected: f32,
) {
    let residential_table = table(&[(0x40, population as *const () as usize)]);
    let mut residential = [residential_table.as_ptr() as u32, population_value as u32];
    let allocator_table = table(&[
        (0x0C, allocate as *const () as usize),
        (0x14, acquire as *const () as usize),
    ]);
    let mut allocator_object = [allocator_table.as_ptr() as u32];
    let sim_table = table(&[(0xDC, capacity as *const () as usize)]);
    let occupant_table = table(&[(0x18, holder as *const () as usize)]);
    let mut sim = Object::new(0x240);
    let view = unsafe { RecyclingView::from_ptr(sim.ptr()) };
    unsafe { sim.ptr().cast::<u32>().write(sim_table.as_ptr() as u32) };
    view.set_residential(residential.as_mut_ptr().cast());
    view.set_count(count);
    let mut sentinel = Box::new([0u32; 3]);
    let end = sentinel.as_mut_ptr() as u32;
    *sentinel = [end, end, 0];
    let mut nodes: Vec<Box<[u32; 3]>> = vec![];
    let mut occupants: Vec<Box<Occupant>> = vec![];
    let mut previous = end;
    for &(id, capacity, served, has_served) in facilities {
        let mut o = Box::new(Occupant {
            table: occupant_table.as_ptr(),
            id,
            capacity,
            served,
            has_served,
        });
        let mut n = Box::new([end, previous, (&mut *o as *mut Occupant) as u32]);
        let p = n.as_mut_ptr() as u32;
        unsafe { ListNode::from_ptr(previous as *mut u8).set_next(p as *mut u8) };
        sentinel[1] = p;
        previous = p;
        nodes.push(n);
        occupants.push(o);
    }
    view.set_recycling(end as *mut u8);
    let mut allocator_pointer = if lazy {
        core::ptr::null_mut()
    } else {
        allocator_object.as_mut_ptr().cast()
    };
    let mut default = 0.125f32;
    STATE.with(|s| {
        *s.borrow_mut() = State {
            calls: vec![],
            allocations: vec![],
            allocator: allocator_object.as_mut_ptr().cast(),
            fail,
            fail_after_sort: late_failure,
            sorted: false,
        }
    });
    let _fakes = [
        fake_data(0x00B2_07E4, (&mut allocator_pointer as *mut *mut u8).cast()),
        fake_data(0x00B4_9BC4, (&mut default as *mut f32).cast()),
        fake(0x0087_93EC, application as *const () as usize),
        fake(0x0041_1870, release as *const () as usize),
        fake(0x006E_5660, assign as *const () as usize),
        fake(0x0041_3FE0, destroy as *const () as usize),
        fake(0x005F_D4F0, served as *const () as usize),
    ];
    unsafe { fc4p_006e6da0(sim.ptr()) };
    assert_eq!(view.effect().to_bits(), expected.to_bits());
    assert_eq!(allocator_pointer, allocator_object.as_mut_ptr().cast());
    let source = unsafe { list_values(view.recycling()) };
    if count != 0 {
        assert!(source.len() <= facilities.len());
    }
}

#[test]
fn default_effect_empty_count_empty_list_and_nonpositive_population() {
    let _fpu = GameFpu::new();
    run(1000, &[(1, 0.5, 600.0, true)], None, false, true, 0, 0.125);
    assert_eq!(
        take_calls(),
        [
            Call::Population,
            Call::Application,
            Call::Acquire(0x3AE4_BEA3, 0x3AE4_BEAB),
            Call::Release,
            Call::Allocate,
            Call::Destroy(vec![])
        ]
    );
    for population in [0, -1, i32::MIN] {
        run(population, &[(1, 0.5, 600.0, true)], None, false, false, 1, 0.125);
        assert!(!take_calls().iter().any(|c| matches!(c, Call::Served(_))));
    }
    run(1000, &[], None, false, false, 1, 0.125);
    assert_eq!(
        take_calls(),
        [
            Call::Population,
            Call::Allocate,
            Call::Assign(vec![]),
            Call::Destroy(vec![])
        ]
    );
}

#[test]
fn equal_capacities_reverse_order_and_missing_queries_skip_only_their_work() {
    let _fpu = GameFpu::new();
    run(
        500,
        &[(1, 0.5, 600.0, true), (2, 0.5, 600.0, true)],
        None,
        false,
        false,
        2,
        0.5,
    );
    let calls = take_calls();
    assert!(calls.contains(&Call::Assign(vec![2, 1])));
    assert!(!calls.contains(&Call::Served(1)));
    run(
        1000,
        &[(1, 0.2, 600.0, true), (2, 0.5, 600.0, true)],
        Some(2),
        false,
        false,
        2,
        0.12,
    );
    assert!(take_calls().contains(&Call::Assign(vec![1])));
    run(1000, &[(1, 0.5, 600.0, false)], None, false, false, 1, 0.125);
    assert_eq!(
        take_calls().iter().filter(|c| matches!(c, Call::Capacity(_))).count(),
        1
    );
    run(1000, &[(1, 0.5, 600.0, true)], None, true, false, 1, 0.0);
    take_calls();
    for served in [0.0, -10.0, f32::NAN] {
        run(1000, &[(1, 0.5, served, true)], None, false, false, 1, 0.125);
        take_calls();
    }
}
