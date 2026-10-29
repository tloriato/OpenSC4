// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    ClearSteps,
    ClearCache,
    Grow([u32; 2]),
    ClearAnchors,
    Push([u32; 2]),
    Draw([u32; 2], [u32; 2], u32),
    Place(u32, u32),
    FilterAllocate,
    FilterConstruct,
    AddRef(usize),
    Release(usize),
    Allocate(u32),
    Free(usize),
    Lookup([u32; 2]),
    IsType(usize, u32),
    Cache(u32),
    Callback([u32; 4], u32),
}

#[derive(Default)]
struct State {
    tool: usize,
    filter: usize,
    callback: usize,
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    neighbours: Vec<([u32; 2], usize, u32)>,
    cache: Vec<(u32, usize)>,
    draw_results: Vec<u32>,
    place_results: Vec<u32>,
    draw_count: usize,
    place_count: usize,
    failure: u32,
    clear_real: Option<u32>,
    grow_real: Option<u32>,
    release_end: usize,
    argument: usize,
    expected_mask: u32,
    change_type: bool,
    result_at_placement: Vec<u8>,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "C" fn clear_steps_fake(first: *mut u8, last: *mut u8, output: *mut u8, real: *mut u32, flags: u32) -> *mut u8 {
    assert_eq!(first, last);
    assert_eq!(flags, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::ClearSteps);
        let tool = unsafe { NetworkTool::from_ptr(s.tool as *mut u8) };
        assert_eq!(tool.insertion_data(), 3);
        assert_eq!(unsafe { DragTool::from_ptr(s.tool as *mut u8) }.failure(), 0);
        s.argument = real as usize;
        if let Some(value) = s.clear_real {
            unsafe {
                *real = value;
            }
        }
        assert_eq!(unsafe { real.add(1).read() } as u8, 1);
    });
    output
}

extern "thiscall" fn clear_cache_fake(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool + 0x1C);
        assert_eq!(unsafe { DragTool::from_ptr(s.tool as *mut u8) }.dragged(), 0);
        assert_eq!(unsafe { DragTool::from_ptr(s.tool as *mut u8) }.straight(), 0);
        s.calls.push(Call::ClearCache);
        if s.change_type {
            unsafe {
                NetworkTool::from_ptr(s.tool as *mut u8).set_network_type(12);
            }
        }
    });
}

extern "thiscall" fn draw(this: *mut u8, a: *mut u32, b: *mut u32, flags: u32, network: u32) -> u32 {
    assert_eq!(flags, 0);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        let a = unsafe { [*a, *a.add(1)] };
        let b = unsafe { [*b, *b.add(1)] };
        let drag = unsafe { DragTool::from_ptr(this) };
        let stored = unsafe { core::slice::from_raw_parts(drag.anchors().cast::<u32>(), 4) };
        assert_eq!(stored, [a[0], a[1], b[0], b[1]]);
        assert_eq!(drag.anchors_end() as usize, drag.anchors() as usize + 16);
        s.calls.push(Call::Draw(a, b, network));
        drag.set_failure(s.failure);
        let i = s.draw_count;
        s.draw_count += 1;
        s.draw_results.get(i).copied().unwrap_or(1)
    })
}

extern "thiscall" fn place_fake(this: *mut u8, real: u32, crew: u32, flags: u32, neighbours: u32, network: u32) -> u32 {
    assert_eq!((crew, flags, neighbours), (0, 0, 0));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        assert_eq!(unsafe { NetworkTool::from_ptr(this) }.insertion_data(), 3);
        s.calls.push(Call::Place(real, network));
        let byte = unsafe { (s.argument as *const u32).add(1).read() } as u8;
        s.result_at_placement.push(byte);
        unsafe {
            NetworkTool::from_ptr(this).set_real(real as u8);
            DragTool::from_ptr(this).set_failure(s.failure);
        }
        let i = s.place_count;
        s.place_count += 1;
        s.place_results.get(i).copied().unwrap_or(1)
    })
}

extern "C" fn filter_alloc(size: u32) -> *mut u8 {
    assert_eq!(size, 16);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::FilterAllocate);
        s.filter as *mut u8
    })
}
extern "thiscall" fn filter_construct(this: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::FilterConstruct));
    this
}
extern "thiscall" fn addref(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::AddRef(this as usize)));
}
extern "thiscall" fn release(this: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Release(this as usize));
        if s.release_end != 0 {
            unsafe {
                DragTool::from_ptr(s.tool as *mut u8).set_anchors_end(s.release_end as *mut u8);
            }
        }
    });
}

extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        assert_eq!(size, 24);
        let mut allocation = vec![0xCCCC_CCCC; 6].into_boxed_slice();
        let p = allocation.as_mut_ptr().cast();
        s.allocations.push(allocation);
        p
    })
}
extern "C" fn free_fake(p: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p as usize)));
}

extern "thiscall" fn grow(this: *mut u8, end: *mut u8, point: *mut u32, real: *mut u32, count: u32, copy: u32) {
    assert_eq!((count, copy), (1, 1));
    unsafe {
        assert_eq!(read::<*mut u8>(this, 4), end);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            assert_eq!(this as usize, s.tool + 0x118);
            assert_eq!(real as usize, s.argument);
            let point = [*point, *point.add(1)];
            s.calls.push(Call::Grow(point));
            let begin: *mut u8 = read(this, 0);
            let size = (end as usize - begin as usize) / 4;
            let mut allocation = vec![0u32; (size + 2).max(2)].into_boxed_slice();
            if size != 0 {
                allocation[..size].copy_from_slice(core::slice::from_raw_parts(begin.cast::<u32>(), size));
            }
            allocation[size] = point[0];
            allocation[size + 1] = point[1];
            let begin = allocation.as_mut_ptr().cast::<u8>();
            write(this, 0, begin);
            write(this, 4, begin.add((size + 2) * 4));
            write(this, 8, begin.add((size + 2) * 4));
            s.allocations.push(allocation);
            if let Some(value) = s.grow_real {
                *real = value;
            }
        });
    }
}

extern "thiscall" fn clear_anchors_fake(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::ClearAnchors));
    unsafe {
        write(this, 4, read::<*mut u8>(this, 0));
    }
}
extern "thiscall" fn push(this: *mut u8, point: *mut u32) {
    unsafe {
        let pair = [*point, *point.add(1)];
        STATE.with(|s| s.borrow_mut().calls.push(Call::Push(pair)));
        let end: *mut u8 = read(this, 4);
        write(end, 0, pair[0]);
        write(end, 4, pair[1]);
        write(this, 4, end.add(8));
    }
}

extern "thiscall" fn lookup(this: *mut u8, x: u32, z: u32, filter: *mut u8, output: *mut *mut u8) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        assert_eq!(filter as usize, s.filter);
        assert_eq!(unsafe { read::<u32>(filter, 12) }, s.expected_mask);
        assert!(unsafe { output.read() }.is_null());
        s.calls.push(Call::Lookup([x, z]));
        let hit = s.neighbours.iter().find(|&&(p, _, _)| p == [x, z]);
        if let Some(&(_, occupant, _)) = hit {
            unsafe {
                output.write(occupant as *mut u8);
            }
            0xABCD_0001
        } else {
            0xABCD_0000
        }
    })
}
extern "thiscall" fn is_type(this: *mut u8, network: u32) -> u32 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::IsType(this as usize, network));
        s.neighbours.iter().find(|&&(_, p, _)| p == this as usize).unwrap().2
    })
}
extern "thiscall" fn cached(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool + 0x1C);
        s.calls.push(Call::Cache(key));
        s.cache
            .iter()
            .find(|&&(k, _)| k == key)
            .map_or(null_mut(), |&(_, p)| p as *mut u8)
    })
}

extern "thiscall" fn callback(
    this: *mut u8,
    x1: u32,
    z1: u32,
    x2: u32,
    z2: u32,
    mask: u32,
    other: u32,
    real: u32,
    network: u32,
) {
    assert_eq!((mask, other, network), (0, 0, u32::MAX));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.callback);
        s.calls.push(Call::Callback([x1, z1, x2, z2], real));
        assert_eq!(unsafe { NetworkTool::from_ptr(s.tool as *mut u8) }.insertion_data(), 3);
    });
}

struct World {
    object: Object,
    anchors: Box<[u32; 8]>,
    steps: Box<[u32; 6]>,
    filter: Box<[u32; 4]>,
    occupants: [[usize; 1]; 2],
    callback: Box<[usize; 1]>,
    _vt: Box<[usize; 19]>,
    _filter_vt: Box<[usize; 3]>,
    _occupant_vt: Box<[usize; 23]>,
    _callback_vt: Box<[usize; 8]>,
    _nx: Box<[u32; 4]>,
    _nz: Box<[u32; 4]>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            object: Object::new(0x370),
            anchors: Box::new([0; 8]),
            steps: Box::new([0; 6]),
            filter: Box::new([0; 4]),
            occupants: [[0]; 2],
            callback: Box::new([0]),
            _vt: Box::new([0; 19]),
            _filter_vt: Box::new([0; 3]),
            _occupant_vt: Box::new([0; 23]),
            _callback_vt: Box::new([0; 8]),
            _nx: Box::new([u32::MAX, 0, 1, 0]),
            _nz: Box::new([0, u32::MAX, 0, 1]),
            _fakes: vec![
                fake(0x0061_43E0, clear_steps_fake as *const () as usize),
                fake(0x0064_7B20, clear_cache_fake as *const () as usize),
                fake(0x007C_61A0, grow as *const () as usize),
                fake(0x0060_6870, clear_anchors_fake as *const () as usize),
                fake(0x0044_67A0, push as *const () as usize),
                fake(0x0063_B830, place_fake as *const () as usize),
                fake(0x005E_55E0, filter_alloc as *const () as usize),
                fake(0x0061_F510, filter_construct as *const () as usize),
                fake(0x0090_CF54, alloc as *const () as usize),
                fake(0x0090_CF63, free_fake as *const () as usize),
                fake(0x0062_4B70, lookup as *const () as usize),
                fake(0x0064_7A20, cached as *const () as usize),
            ],
        };
        w._fakes.push(fake_data(0x00AA_8390, w._nx.as_mut_ptr().cast()));
        w._fakes.push(fake_data(0x00AA_83A0, w._nz.as_mut_ptr().cast()));
        w._vt[18] = draw as *const () as usize;
        w._filter_vt[1] = addref as *const () as usize;
        w._filter_vt[2] = release as *const () as usize;
        w._occupant_vt[2] = release as *const () as usize;
        w._occupant_vt[22] = is_type as *const () as usize;
        w._callback_vt[7] = callback as *const () as usize;
        w.filter[0] = w._filter_vt.as_ptr() as u32;
        w.occupants
            .iter_mut()
            .for_each(|o| o[0] = w._occupant_vt.as_ptr() as usize);
        w.callback[0] = w._callback_vt.as_ptr() as usize;
        let t = unsafe { NetworkTool::from_ptr(w.object.ptr()) };
        t.set_vtable(w._vt.as_ptr() as u32);
        t.set_width(64);
        t.set_height(96);
        t.set_network_type(3);
        t.set_insertion_data(1);
        let d = unsafe { DragTool::from_ptr(w.object.ptr()) };
        d.set_steps(w.steps.as_mut_ptr().cast());
        d.set_steps_end(w.steps.as_mut_ptr().wrapping_add(6).cast());
        d.set_dragged(9);
        d.set_straight(10);
        d.set_failure(0xF000_0002);
        d.set_anchors(w.anchors.as_mut_ptr().cast());
        d.set_anchors_end(w.anchors.as_mut_ptr().wrapping_add(6).cast());
        d.set_anchors_capacity(w.anchors.as_mut_ptr().wrapping_add(8).cast());
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.tool = w.object.ptr() as usize;
            s.filter = w.filter.as_mut_ptr() as usize;
            s.callback = w.callback.as_mut_ptr() as usize;
        });
        w
    }
    fn run(&mut self, coords: [u32; 4], mask: u32, real: u32, network: u32, callback: bool) -> (u32, u32) {
        let cb = if callback { self.callback.as_mut_ptr() as u32 } else { 0 };
        let args = [coords[0], coords[1], coords[2], coords[3], mask, cb, real, network];
        STATE.with(|s| s.borrow_mut().expected_mask = mask);
        unsafe {
            call_thiscall_with_eax(
                fc4p_00629210 as *const () as usize,
                self.object.ptr(),
                &args,
                0xABCD_EF00,
            )
        }
    }
    fn neighbour(&mut self, point: [u32; 2], occupant: usize, same: u32) {
        let p = self.occupants[occupant].as_mut_ptr() as usize;
        STATE.with(|s| s.borrow_mut().neighbours.push((point, p, same)));
    }
    fn drag(&mut self) -> DragTool {
        unsafe { DragTool::from_ptr(self.object.ptr()) }
    }
}

fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn straight_run_clamps_coordinates_and_restores_insertion_data() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.place_results = vec![0x1234_007F];
        s.change_type = true;
    });
    let result = w.run(
        [(-8i32) as u32, 200, 140, (-7i32) as u32],
        0,
        0xCAFE_0001,
        u32::MAX,
        false,
    );
    assert_eq!(result, ((w.anchors.as_ptr().wrapping_add(4) as u32 & !0xFF) | 0x7F, 0));
    assert_eq!(
        calls(),
        [
            Call::ClearSteps,
            Call::ClearCache,
            Call::Draw([0, 95], [63, 0], 3),
            Call::Place(0xCAFE_0001, 3),
        ]
    );
    let drag = w.drag();
    assert_eq!(drag.steps_end(), drag.steps());
    assert_eq!(drag.anchors_end(), drag.anchors());
    assert_eq!((drag.dragged(), drag.straight(), drag.failure()), (0, 0, 0));
    assert_eq!(unsafe { NetworkTool::from_ptr(w.object.ptr()) }.insertion_data(), 1);
}

#[test]
fn failed_draw_skips_placement_and_failure_word_overrides_a_success() {
    for failure in [0, 0xF000_0002] {
        let mut w = World::new();
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.draw_results = vec![if failure == 0 { 0x1234_0000 } else { 1 }];
            s.failure = failure;
        });
        assert_eq!(w.run([10, 10, 20, 10], 0, 0, 8, false).0 as u8, 0);
        let log = calls();
        assert!(!log.contains(&Call::ClearCache));
        assert_eq!(
            log.iter().filter(|c| matches!(c, Call::Place(..))).count(),
            usize::from(failure != 0)
        );
        assert_eq!(w.drag().failure(), failure);
    }
}

#[test]
fn masked_diagonal_only_constructs_and_releases_filter() {
    let mut w = World::new();
    let old_end = w.drag().anchors_end() as u32;
    let filter = w.filter.as_ptr() as usize;
    assert_eq!(w.run([5, 5, 8, 9], 0xC49, 1, 3, true), ((old_end & !0xFF) | 1, 0));
    assert_eq!(
        calls(),
        [
            Call::ClearSteps,
            Call::ClearCache,
            Call::FilterAllocate,
            Call::FilterConstruct,
            Call::AddRef(filter),
            Call::Release(filter)
        ]
    );
    assert_eq!(w.filter[3], 0xC49);
}

#[test]
fn masked_straight_run_extends_to_matching_occupants() {
    let mut w = World::new();
    w.neighbour([10, 10], 0, 0xFFFF_FF01);
    w.neighbour([15, 10], 1, 1);
    assert_eq!(w.run([11, 10, 14, 10], 0xC49, 1, 3, false).0 as u8, 1);
    let log = calls();
    assert!(log.contains(&Call::Draw([10, 10], [15, 10], 3)));
    let a = w.occupants[0].as_ptr() as usize;
    let b = w.occupants[1].as_ptr() as usize;
    let start = log.iter().position(|c| *c == Call::Lookup([10, 10])).unwrap();
    assert_eq!(
        &log[start..start + 5],
        [
            Call::Lookup([10, 10]),
            Call::IsType(a, 3),
            Call::Release(a),
            Call::Lookup([15, 10]),
            Call::IsType(b, 3)
        ]
    );
    assert!(!log.iter().any(|c| matches!(c, Call::Cache(..) | Call::Callback(..))));
}

#[test]
fn joins_are_called_in_order_after_placement_and_before_cleanup() {
    let mut w = World::new();
    w.neighbour([10, 10], 0, 0xABCD_0000);
    w.neighbour([15, 10], 1, 0);
    w.run([11, 10, 14, 10], 0xC49, 0xCAFE_0001, 3, true);
    let log = calls();
    let place = log.iter().position(|c| matches!(c, Call::Place(..))).unwrap();
    assert_eq!(
        &log[place..place + 3],
        [
            Call::Place(0xCAFE_0001, 3),
            Call::Callback([11, 10, 10, 10], 0xCAFE_0001),
            Call::Callback([15, 10, 14, 10], 0xCAFE_0001)
        ]
    );
    assert_eq!(log.iter().filter(|c| **c == Call::Allocate(24)).count(), 3);
    let allocations = STATE.with(|s| {
        s.borrow()
            .allocations
            .iter()
            .map(|a| a.as_ptr() as usize)
            .collect::<Vec<_>>()
    });
    let frees = log
        .iter()
        .filter_map(|c| if let Call::Free(p) = c { Some(*p) } else { None })
        .collect::<Vec<_>>();
    assert_eq!(frees, [allocations[1], allocations[2], allocations[0]]);
}

#[test]
fn preview_still_records_other_type_joins_but_omits_callback() {
    let mut w = World::new();
    w.neighbour([10, 10], 0, 0);
    w.run([11, 10, 14, 10], 0xC49, 0, 3, true);
    let log = calls();
    assert_eq!(log.iter().filter(|c| **c == Call::Allocate(24)).count(), 2);
    assert!(!log.iter().any(|c| matches!(c, Call::Callback(..))));
}

#[test]
fn draft_extends_through_matching_cache_flags_only_after_a_miss() {
    let mut w = World::new();
    let mut first = [0u32, 0, 0xC49];
    let mut last = [0u32, 0, 0x8000_0000];
    STATE.with(|s| {
        s.borrow_mut().cache = vec![
            (0xA000A, first.as_mut_ptr() as usize),
            (0xA000F, last.as_mut_ptr() as usize),
        ]
    });
    w.run([11, 10, 14, 10], 0xC49, 0, 3, false);
    let log = calls();
    assert!(log.contains(&Call::Draw([10, 10], [14, 10], 3)));
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Cache(..))).collect::<Vec<_>>(),
        [&Call::Cache(0xA000A), &Call::Cache(0xA000F)]
    );
}

#[test]
fn single_cell_without_neighbours_uses_clear_and_push_helpers() {
    let mut w = World::new();
    w.run([10, 10, 10, 10], 0xC49, 1, 3, false);
    let log = calls();
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Lookup(..))).collect::<Vec<_>>(),
        [
            &Call::Lookup([9, 10]),
            &Call::Lookup([10, 9]),
            &Call::Lookup([11, 10]),
            &Call::Lookup([10, 11]),
        ]
    );
    let clear = log.iter().position(|c| *c == Call::ClearAnchors).unwrap();
    assert_eq!(
        &log[clear..clear + 5],
        [
            Call::ClearAnchors,
            Call::Push([10, 10]),
            Call::Push([10, 10]),
            Call::Draw([10, 10], [10, 10], 3),
            Call::Place(1, 3)
        ]
    );
}

#[test]
fn single_cell_neighbours_on_both_axes_create_two_runs_and_last_result_wins() {
    let mut w = World::new();
    w.neighbour([9, 10], 0, 1);
    w.neighbour([10, 9], 1, 1);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.draw_results = vec![0, 1];
        s.place_results = vec![0x80];
    });
    assert_eq!(w.run([10, 10, 10, 10], 0xC49, 1, 3, false).0 as u8, 0x80);
    let log = calls();
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Draw(..))).collect::<Vec<_>>(),
        [&Call::Draw([9, 10], [10, 10], 3), &Call::Draw([10, 9], [10, 10], 3),]
    );
}

#[test]
fn minimum_integer_delta_has_zero_direction_and_clamping_handles_zero_dimensions() {
    let mut w = World::new();
    w.run([0, 4, 0x8000_0000, 4], 1, 1, 3, false);
    let log = calls();
    assert!(log.contains(&Call::Draw([0, 4], [0, 4], 3)));
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Lookup(..))).collect::<Vec<_>>(),
        [&Call::Lookup([0, 4]), &Call::Lookup([0, 4])]
    );
    let mut w = World::new();
    unsafe {
        NetworkTool::from_ptr(w.object.ptr()).set_width(0);
        NetworkTool::from_ptr(w.object.ptr()).set_height(0);
    }
    w.run([3, 4, 5, 6], 0, 0, 3, false);
    assert!(calls().contains(&Call::Draw([0, 0], [0, 0], 3)));
}

#[test]
fn vector_growth_receives_original_real_word_and_placement_uses_its_changed_value() {
    let mut w = World::new();
    w.drag().set_anchors_capacity(w.anchors.as_mut_ptr().cast());
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.grow_real = Some(0xFACE_0001);
    });
    assert_eq!(w.run([1, 2, 3, 4], 0, 0, 7, false).0 as u8, 1);
    let log = calls();
    assert_eq!(
        log,
        [
            Call::ClearSteps,
            Call::Grow([1, 2]),
            Call::Grow([3, 4]),
            Call::Draw([1, 2], [3, 4], 7),
            Call::Place(0xFACE_0001, 7)
        ]
    );
}

#[test]
fn real_word_is_reloaded_after_clear_helper_and_return_keeps_end_pointer_bytes() {
    let mut w = World::new();
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.clear_real = Some(1);
        s.release_end = 0xABCD_EF00;
    });
    assert_eq!(w.run([1, 2, 3, 4], 1, 0, 3, false), (0xABCD_EF01, 0));
    let log = calls();
    assert_eq!(log[0..2], [Call::ClearSteps, Call::ClearCache]);
    assert_eq!(w.drag().anchors_end(), w.drag().anchors());
}

#[test]
fn draw_result_is_stored_before_placement_only_on_the_direct_paths() {
    for (coords, mask, expected) in [([1, 2, 3, 4], 0, 0x80), ([1, 2, 3, 2], 1, 1), ([1, 2, 1, 2], 1, 0x80)] {
        let mut w = World::new();
        STATE.with(|s| s.borrow_mut().draw_results = vec![0x80]);
        w.run(coords, mask, 1, 3, false);
        assert_eq!(STATE.with(|s| s.borrow().result_at_placement.clone()), [expected]);
    }
}

#[test]
fn direct_placement_writes_only_step_counts_anchor_end_and_request_fields() {
    let mut w = World::new();
    let before = unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x370).to_vec() };
    w.run([1, 2, 3, 4], 0, 0, 3, false);
    let after = unsafe { core::slice::from_raw_parts(w.object.ptr(), 0x370) };
    for (offset, (&a, &b)) in before.iter().zip(after).enumerate() {
        if !((0x58..0x5C).contains(&offset)
            || (0x70..0x78).contains(&offset)
            || (0x11C..0x120).contains(&offset)
            || (0x208..0x210).contains(&offset))
        {
            assert_eq!(a, b, "offset {offset:x}");
        }
    }
}
