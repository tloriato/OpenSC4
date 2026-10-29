// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{call_raw, Conv, RawCall};
use crate::testing::{fake, fake_data, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Free(usize),
    Cell(u32),
    Erase(usize, [u32; 2]),
    Selection(usize, [u32; 2], u32),
    Portal(u32, u32, usize),
    Query(usize),
    Pair(usize, usize),
    Manager(usize),
    Connect(usize, usize, usize),
    Release(usize),
}

#[derive(Default)]
struct State {
    tool: usize,
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    cells: Vec<(u32, usize)>,
    portals: Vec<usize>,
    inserted: usize,
    interfaces: Vec<(usize, usize, u32)>,
    managers: Vec<(usize, usize)>,
    preview_replacement: usize,
    change_step: usize,
    change_end: u32,
    change_on_allocate: usize,
    stop_real_after: usize,
    stop_real_on_lookup: usize,
    lookups: usize,
    change_preview_cell: bool,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        assert_eq!(size, 12);
        if s.change_on_allocate != 0 {
            unsafe {
                write(s.change_on_allocate as *mut u8, 4, 1u32);
            }
            s.change_on_allocate = 0;
        }
        let mut allocation = vec![0xCDCD_CDCD; 3].into_boxed_slice();
        let p = allocation.as_mut_ptr().cast();
        s.allocations.push(allocation);
        p
    })
}

extern "C" fn free_fake(p: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p as usize)));
    registers(0xCCCC_BBBB, 0xAABB_CCDD)
}

extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        s.calls.push(Call::Cell(key));
        s.lookups += 1;
        if s.stop_real_on_lookup == s.lookups {
            unsafe {
                NetworkTool::from_ptr(this).set_real(0);
            }
        }
        s.cells.iter().find(|&&(k, _)| k == key).unwrap().1 as *mut u8
    })
}

extern "thiscall" fn erase(this: *mut u8, cell: *mut u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Erase(this as usize, unsafe { [*cell, *cell.add(1)] }));
        if s.change_preview_cell {
            unsafe {
                *cell = 0xFEDC;
                *cell.add(1) = 0xBA98;
            }
        }
        if s.preview_replacement != 0 {
            unsafe {
                NetworkTool::from_ptr(s.tool as *mut u8).set_preview(s.preview_replacement as *mut u8);
            }
        }
    });
}

extern "thiscall" fn selection(this: *mut u8, cell: *mut u32, color: u32, heights: u32, layer: u32, flags: u32) {
    assert_eq!((color, heights, flags), (0x8000_FF00, 0, 0));
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls
            .push(Call::Selection(this as usize, unsafe { [*cell, *cell.add(1)] }, layer));
        if s.change_step != 0 {
            unsafe {
                write(s.change_step as *mut u8, 4, s.change_end);
            }
            s.change_step = 0;
        }
    });
}

extern "thiscall" fn portal(this: *mut u8, direction: u32, index: u32, cell: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        assert_eq!(unsafe { read::<u8>(cell, 0x51) }, 1);
        s.calls.push(Call::Portal(direction, index, cell as usize));
        let occupant = s.portals[s.inserted];
        s.inserted += 1;
        if s.inserted == s.stop_real_after {
            unsafe {
                NetworkTool::from_ptr(this).set_real(0);
            }
        }
        occupant as *mut u8
    })
}

extern "thiscall" fn query(this: *mut u8, class: u32, output: *mut *mut u8) -> u32 {
    assert_eq!(class, 0x8A4B_D52B);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Query(this as usize));
        let &(_, interface, result) = s.interfaces.iter().find(|&&(p, _, _)| p == this as usize).unwrap();
        unsafe {
            assert!((*output).is_null());
            *output = interface as *mut u8;
        }
        result
    })
}

extern "thiscall" fn pair(this: *mut u8, other: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Pair(this as usize, other as usize)));
}

extern "thiscall" fn manager(this: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Manager(this as usize));
        s.managers.iter().find(|&&(p, _)| p == this as usize).unwrap().1 as *mut u8
    })
}

extern "thiscall" fn connect(this: *mut u8, a: *mut u8, b: *mut u8) {
    STATE.with(|s| {
        s.borrow_mut()
            .calls
            .push(Call::Connect(this as usize, a as usize, b as usize))
    });
}

extern "thiscall" fn release(this: *mut u8) {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Release(this as usize)));
}

// Each object keeps its address when additional stand-ins are appended to the vectors.
#[allow(clippy::vec_box)]
struct World {
    object: Object,
    records: Vec<[u32; 4]>,
    steps: Vec<[u32; 3]>,
    coordinates: Vec<[u32; 2]>,
    cells: Vec<Box<[u8; 0xB8]>>,
    occupants: Vec<Box<[usize; 1]>>,
    interfaces: Vec<Box<[usize; 1]>>,
    manager: Box<[usize; 1]>,
    _vt: Box<[usize; 50]>,
    _manager_vt: Box<[usize; 33]>,
    _layers: Box<[u32; 13]>,
    _fakes: Vec<Fake>,
}

impl World {
    fn new(points: Vec<[u32; 2]>, steps: Vec<[u32; 3]>, records: Vec<[u32; 4]>, real: bool) -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            object: Object::new(0x370),
            records,
            steps,
            coordinates: points,
            cells: Vec::new(),
            occupants: Vec::new(),
            interfaces: Vec::new(),
            manager: Box::new([0]),
            _vt: Box::new([0; 50]),
            _manager_vt: Box::new([0; 33]),
            _layers: Box::new(core::array::from_fn(|i| 0x7000 + i as u32)),
            _fakes: vec![
                fake(0x0063_3220, lookup as *const () as usize),
                fake(0x0062_0CF0, erase as *const () as usize),
                fake(0x0062_0820, selection as *const () as usize),
                fake(0x0090_CF54, alloc as *const () as usize),
                fake(0x0090_CF63, free_fake as *const () as usize),
                fake(0x0062_8390, portal as *const () as usize),
                fake(0x0064_7530, pair as *const () as usize),
            ],
        };
        w._fakes.push(fake_data(0x00AA_83F0, w._layers.as_mut_ptr().cast()));
        w._vt[0] = query as *const () as usize;
        w._vt[2] = release as *const () as usize;
        w._vt[49] = manager as *const () as usize;
        w._manager_vt[32] = connect as *const () as usize;
        w.manager[0] = w._manager_vt.as_ptr() as usize;
        let tool = unsafe { NetworkTool::from_ptr(w.object.ptr()) };
        tool.set_real(u8::from(real));
        tool.set_preview(0x1234 as *mut u8);
        tool.set_tunnel_begin(w.records.as_mut_ptr().cast());
        tool.set_tunnel_end(w.records.as_mut_ptr().wrapping_add(w.records.len()).cast());
        let drag = unsafe { TunnelInsertionTool::from_ptr(w.object.ptr()) };
        drag.set_steps(w.steps.as_mut_ptr().cast());
        drag.set_cells(w.coordinates.as_mut_ptr().cast());
        drag.set_valid_steps(w.steps.len() as i32);
        STATE.with(|s| s.borrow_mut().tool = w.object.ptr() as usize);
        for point in &w.coordinates {
            let mut cell = Box::new([0xCCu8; 0xB8]);
            cell[0x51] = 0xA5;
            STATE.with(|s| {
                s.borrow_mut()
                    .cells
                    .push((cell_key(*point), cell.as_mut_ptr() as usize))
            });
            w.cells.push(cell);
        }
        for _ in 0..16 {
            let mut occupant = Box::new([w._vt.as_ptr() as usize]);
            let mut interface = Box::new([w._vt.as_ptr() as usize]);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.portals.push(occupant.as_mut_ptr() as usize);
                s.interfaces.push((
                    occupant.as_mut_ptr() as usize,
                    interface.as_mut_ptr() as usize,
                    0x1234_0001,
                ));
                s.managers
                    .push((interface.as_mut_ptr() as usize, w.manager.as_mut_ptr() as usize));
            });
            w.occupants.push(occupant);
            w.interfaces.push(interface);
        }
        w
    }
    fn run(&mut self, layer: u32) -> (u32, u32) {
        // The body receives the saved direction bytes separately from the layer selector.
        let mut frame = [0xABCD_EF55, 0x1111_2222, 0x3333_4444, 0, layer];
        let result = unsafe { insert(self.object.ptr(), frame.as_mut_ptr()) };
        (result as u32, (result >> 32) as u32)
    }
    fn one_pair(real: bool) -> Self {
        Self::new(
            vec![[1, 2], [3, 2]],
            vec![[0, 0, 0], [1, 1, 0]],
            vec![[0, 0, 0, 1]],
            real,
        )
    }
    fn cell(&self, i: usize) -> usize {
        self.cells[i].as_ptr() as usize
    }
    fn occupant(&self, i: usize) -> usize {
        self.occupants[i].as_ptr() as usize
    }
    fn interface(&self, i: usize) -> usize {
        self.interfaces[i].as_ptr() as usize
    }
}

fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn empty_and_out_of_range_spans_preserve_edx() {
    let mut w = World::new(vec![], vec![], vec![], true);
    assert_eq!(w.run(0), (w.records.as_ptr() as u32, 0x3333_4444));
    assert_eq!(calls(), []);
    for real in [false, true] {
        let mut w = World::one_pair(real);
        w.records[0][3] = 2;
        assert_eq!(w.run(0), (2, 0x3333_4444));
        assert_eq!(calls(), []);
    }
}

#[test]
fn previews_inclusive_ranges_reloads_preview_and_bounds() {
    let mut w = World::new(
        vec![[1, 2], [3, 4], [5, 6]],
        vec![[0, 1, 0], [2, 2, 0]],
        vec![[0, 0, 0, 1]],
        false,
    );
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.preview_replacement = 0x5678;
        s.change_step = w.steps.as_mut_ptr() as usize;
        s.change_end = 0;
    });
    let result = w.run(12);
    assert_eq!(
        result,
        (w.records.as_ptr().wrapping_add(1) as u32, w.records.as_ptr() as u32)
    );
    assert_eq!(
        calls(),
        [
            Call::Erase(0x1234, [1, 2]),
            Call::Selection(0x5678, [1, 2], 0x700C),
            Call::Erase(0x5678, [5, 6]),
            Call::Selection(0x5678, [5, 6], 0x700C),
        ]
    );
    assert!(w.cells.iter().all(|c| c[0x51] == 0xA5));
}

#[test]
fn preview_with_empty_step_range_does_not_allocate_or_change_edx() {
    let mut w = World::one_pair(false);
    w.records[0][2] = 1;
    w.records[0][3] = 0;
    assert_eq!(w.run(0).1, 0x3333_4444);
    assert_eq!(calls(), []);
}

#[test]
fn preview_erase_can_change_the_coordinates_used_by_selection() {
    let mut w = World::one_pair(false);
    STATE.with(|s| s.borrow_mut().change_preview_cell = true);
    w.run(0);
    assert_eq!(
        calls(),
        [
            Call::Erase(0x1234, [1, 2]),
            Call::Selection(0x1234, [0xFEDC, 0xBA98], 0x7000),
            Call::Erase(0x1234, [3, 2]),
            Call::Selection(0x1234, [0xFEDC, 0xBA98], 0x7000),
        ]
    );
}

#[test]
fn directions_follow_signed_endpoint_comparisons_and_pair_in_reverse_order() {
    for (first, last, direction, first_indices, last_indices) in [
        ([0, 0], [4, 0], 1, [0, 1], [3, 2]),
        ([4, 0], [0, 0], 3, [1, 0], [2, 3]),
        ([0, 0], [0, 4], 2, [1, 0], [2, 3]),
        ([0, 4], [0, 0], 0, [0, 1], [3, 2]),
        ([0, 0], [u32::MAX, 0], 3, [1, 0], [2, 3]),
    ] {
        let mut w = World::new(
            vec![first, [first[0], first[1] + 1], last, [last[0], last[1] + 1]],
            vec![[0, 1, 0], [2, 3, 0]],
            vec![[0, 0, 0, 1]],
            true,
        );
        assert_eq!(w.run(0).1, 0xAABB_CCDD);
        let log = calls();
        let portals: Vec<_> = log.iter().filter(|c| matches!(c, Call::Portal(..))).collect();
        assert_eq!(
            portals,
            [
                &Call::Portal(0xABCD_EF00 | direction, 0, w.cell(first_indices[0])),
                &Call::Portal(0xABCD_EF00 | direction, 1, w.cell(first_indices[1])),
                &Call::Portal(direction.wrapping_sub(2) & 3, 0, w.cell(last_indices[0])),
                &Call::Portal(direction.wrapping_sub(2) & 3, 1, w.cell(last_indices[1])),
            ]
        );
        let pairs: Vec<_> = log.iter().filter(|c| matches!(c, Call::Pair(..))).collect();
        assert_eq!(
            pairs,
            [
                &Call::Pair(w.interface(1), w.occupant(2)),
                &Call::Pair(w.interface(2), w.occupant(1)),
                &Call::Pair(w.interface(0), w.occupant(3)),
                &Call::Pair(w.interface(3), w.occupant(0)),
            ]
        );
        for cell in &w.cells {
            assert_eq!(cell[0x51], 1);
            assert!(cell.iter().enumerate().all(|(i, &b)| i == 0x51 || b == 0xCC));
        }
        let last = STATE.with(|s| s.borrow().allocations[0].as_ptr() as usize);
        assert_eq!(log.last(), Some(&Call::Free(last)));
    }
}

#[test]
fn successful_pair_calls_managers_in_order_and_releases_both_interfaces() {
    let mut w = World::one_pair(true);
    w.run(0);
    let log = calls();
    let start = log.iter().position(|c| *c == Call::Query(w.occupant(0))).unwrap();
    let a = w.interface(0);
    let b = w.interface(1);
    let manager = w.manager.as_ptr() as usize;
    assert_eq!(
        &log[start..start + 10],
        [
            Call::Query(w.occupant(0)),
            Call::Query(w.occupant(1)),
            Call::Pair(a, w.occupant(1)),
            Call::Pair(b, w.occupant(0)),
            Call::Manager(a),
            Call::Connect(manager, a, b),
            Call::Manager(b),
            Call::Connect(manager, b, a),
            Call::Release(b),
            Call::Release(a),
        ]
    );
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
    assert_eq!(frees, [allocations[1], allocations[0]]);
}

#[test]
fn failed_queries_release_written_outputs_and_skip_pairing() {
    for failed in [0, 1] {
        let mut w = World::one_pair(true);
        STATE.with(|s| s.borrow_mut().interfaces[failed].2 = 0xFFFF_FF00);
        w.run(0);
        let log = calls();
        assert!(!log.iter().any(|c| matches!(c, Call::Pair(..) | Call::Manager(..))));
        let releases: Vec<_> = log.iter().filter(|c| matches!(c, Call::Release(..))).collect();
        if failed == 0 {
            assert_eq!(releases, [&Call::Release(w.interface(0))]);
            assert!(!log.contains(&Call::Query(w.occupant(1))));
        } else {
            assert_eq!(
                releases,
                [&Call::Release(w.interface(1)), &Call::Release(w.interface(0))]
            );
        }
    }
}

#[test]
fn absent_managers_still_pair_and_release() {
    let mut w = World::one_pair(true);
    STATE.with(|s| s.borrow_mut().managers.iter_mut().for_each(|(_, m)| *m = 0));
    w.run(0);
    let log = calls();
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Pair(..))).count(), 2);
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Release(..))).count(), 2);
    assert!(!log.iter().any(|c| matches!(c, Call::Connect(..))));
}

#[test]
fn diagonal_span_stops_before_allocating_or_visiting_later_spans() {
    let mut w = World::new(
        vec![[1, 2], [3, 4]],
        vec![[0, 0, 0], [1, 1, 0]],
        vec![[0, 0, 0, 1], [0, 0, 0, 1]],
        true,
    );
    assert_eq!(w.run(0), (4, 2));
    assert_eq!(calls(), []);
}

#[test]
fn equal_endpoints_use_direction_three_and_the_opposite_direction_one() {
    let mut w = World::new(vec![[1, 2]], vec![[0, 0, 0], [0, 0, 0]], vec![[0, 0, 0, 1]], true);
    w.run(0);
    let log = calls();
    assert_eq!(
        log.iter().filter(|c| matches!(c, Call::Portal(..))).collect::<Vec<_>>(),
        [&Call::Portal(0xABCD_EF03, 0, w.cell(0)), &Call::Portal(1, 0, w.cell(0)),]
    );
}

#[test]
fn invalid_later_span_keeps_the_previous_pass_edx_and_stops() {
    for real in [false, true] {
        let mut w = World::new(
            vec![[1, 2], [3, 2]],
            vec![[0, 0, 0], [1, 1, 0]],
            vec![[0, 0, 0, 1], [0, 0, 0, 2]],
            real,
        );
        let (eax, edx) = w.run(0);
        assert_eq!(eax, 2);
        assert_eq!(edx, if real { 0xAABB_CCDD } else { w.records.as_ptr() as u32 });
        let log = calls();
        assert_eq!(
            log.iter().filter(|c| matches!(c, Call::Portal(..))).count(),
            if real { 2 } else { 0 }
        );
        assert_eq!(
            log.iter().filter(|c| matches!(c, Call::Selection(..))).count(),
            if real { 0 } else { 2 }
        );
    }
}

#[test]
fn real_flag_is_rechecked_after_lookup_and_unpaired_nodes_are_freed() {
    let mut w = World::one_pair(true);
    STATE.with(|s| s.borrow_mut().stop_real_after = 1);
    w.run(0);
    let log = calls();
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Portal(..))).count(), 1);
    assert!(!log.iter().any(|c| matches!(c, Call::Query(..))));
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Free(..))).count(), 2);
    assert!(w.cells.iter().all(|c| c[0x51] == 1));

    let mut w = World::one_pair(true);
    STATE.with(|s| s.borrow_mut().stop_real_on_lookup = 1);
    w.run(0);
    let log = calls();
    assert!(!log.iter().any(|c| matches!(c, Call::Portal(..))));
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Free(..))).count(), 1);
}

#[test]
fn allocator_changes_to_first_range_are_visible() {
    let mut w = World::new(
        vec![[1, 2], [1, 3], [3, 2], [3, 3]],
        vec![[0, 0, 0], [2, 3, 0]],
        vec![[0, 0, 0, 1]],
        true,
    );
    STATE.with(|s| s.borrow_mut().change_on_allocate = w.steps.as_mut_ptr() as usize);
    w.run(0);
    let log = calls();
    assert!(log.contains(&Call::Portal(0xABCD_EF01, 1, w.cell(1))));
    assert_eq!(log.iter().filter(|c| matches!(c, Call::Portal(..))).count(), 4);
}

#[unsafe(naked)]
unsafe extern "C" fn seeded_entry() {
    core::arch::naked_asm!(
        "push eax", "lea eax, [esp]", "and eax, -8", "mov dword ptr [eax - 0x18], 0x87654321", "pop eax",
        "jmp {entry}", entry = sym fc4p_006287d0,
    );
}

#[unsafe(naked)]
unsafe extern "C" fn seeded_entry_shifted() {
    core::arch::naked_asm!(
        "push 0", "push dword ptr [esp + 8]", "call {seeded}", "add esp, 4", "ret 4",
        seeded = sym seeded_entry,
    );
}

#[test]
fn direction_storage_is_preserved_at_the_other_entry_stack_alignment() {
    let mut w = World::one_pair(true);
    let args = [3u32];
    let mut c = RawCall::new(
        Conv::Thiscall,
        seeded_entry_shifted as *const () as usize,
        w.object.ptr() as u32,
        &args,
    );
    c.edx = 0x1234_5678;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!(c.edx, 0xAABB_CCDD);
    assert!(calls().contains(&Call::Portal(0x8765_4301, 0, w.cell(0))));
}

#[test]
fn export_preserves_upper_direction_bytes_entry_edx_and_callee_stack_pop() {
    let mut w = World::one_pair(true);
    let args = [3u32];
    let mut c = RawCall::new(
        Conv::Thiscall,
        seeded_entry as *const () as usize,
        w.object.ptr() as u32,
        &args,
    );
    c.edx = 0x1234_5678;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!(c.edx, 0xAABB_CCDD);
    assert!(calls().contains(&Call::Portal(0x8765_4301, 0, w.cell(0))));

    let mut w = World::new(vec![], vec![], vec![], true);
    c.ecx = w.object.ptr() as u32;
    c.edx = 0x1234_5678;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!(c.edx, 0x1234_5678);
    assert_eq!(c.eax, w.records.as_ptr() as u32);
}
