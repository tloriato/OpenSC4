// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, Fake, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Clear,
    Cell(u32),
    Grow([u32; 2]),
    Allocate(u32),
    Free(usize),
    RegionFree(usize),
    Compute(Vec<[u32; 2]>, u32),
    Swap,
    Erase(usize),
}

#[derive(Default)]
struct State {
    calls: Vec<Call>,
    allocations: Vec<Box<[u32]>>,
    cells: Vec<(u32, usize)>,
    tool: usize,
    success: u32,
    change_argument: bool,
    mutate_record: usize,
    mutate_span: usize,
    replacement_tree: usize,
    replacement_region: usize,
    erase_replacement: usize,
    swap_replacement: usize,
    swap_count: u32,
    new_tree: usize,
    new_count: u32,
}

thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

extern "thiscall" fn clear(this: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        s.calls.push(Call::Clear);
    });
    registers(0x1122_3344, 0x5566_7788)
}

extern "C" fn alloc(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Allocate(size));
        let mut memory = vec![0xA5A5_A5A5; (size as usize).div_ceil(4)].into_boxed_slice();
        let p = memory.as_mut_ptr().cast();
        s.allocations.push(memory);
        p
    })
}

extern "C" fn free_fake(p: *mut u8) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p as usize)));
    registers(0xFEEE_FEEE, 0xED00_0000 | p as u32 & 0x00FF_FFFF)
}

extern "C" fn region_free(p: *mut u8) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::RegionFree(p as usize));
        // The height and the column array can change after each column is freed.
        if s.replacement_region != 0 {
            unsafe {
                let region = HighlightTool::from_ptr(s.tool as *mut u8).region();
                write(region, 0x14, 1u32);
                write(region, 0x20, s.replacement_region as u32);
            }
        }
    });
}

extern "thiscall" fn lookup(this: *mut u8, key: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        assert_eq!(this as usize, s.tool);
        s.calls.push(Call::Cell(key));
        if s.mutate_record != 0 {
            unsafe {
                write(s.mutate_record as *mut u8, 0, 0xCAFEu32);
                write(s.mutate_record as *mut u8, 4, 0xFACEu32);
            }
            s.mutate_record = 0;
        }
        if s.mutate_span != 0 {
            unsafe {
                write(s.mutate_span as *mut u8, 4, 0u32);
                write(this, 0x78, 1i32);
            }
            s.mutate_span = 0;
        }
        s.cells
            .iter()
            .find(|&&(k, _)| k == key)
            .map_or(null_mut(), |&(_, p)| p as *mut u8)
    })
}

extern "thiscall" fn grow(
    this: *mut CellVector,
    end: *mut u8,
    pair: *const u32,
    argument: *mut u32,
    count: u32,
    copy: u32,
) {
    unsafe {
        let pair = [*pair, *pair.add(1)];
        assert_eq!((count, copy), (1, 1));
        assert_eq!(end, (*this).end);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Grow(pair));
            if s.change_argument {
                *argument = 0x8765_4321;
            }
        });
        append_inline(&mut *this, pair);
    }
}

extern "C" fn compute(
    cells: *mut CellVector,
    region: *mut *mut u8,
    cost: *mut u32,
    tree: *mut Tree,
    filter: u32,
) -> u32 {
    unsafe {
        let len = ((*cells).end as usize - (*cells).begin as usize) / 8;
        let mut candidates = Vec::new();
        for i in 0..len {
            let p = (*cells).begin.add(i * 8);
            candidates.push([read(p, 0), read(p, 4)]);
        }
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Compute(candidates, filter));
            let tool = s.tool as *mut u8;
            assert_eq!(region, tool.add(0x210).cast());
            assert_eq!(cost, tool.add(0x220).cast());
            assert!((*region).is_null());
            assert_eq!((*tree).count, 0);
            let head = (*tree).head;
            assert_eq!(read::<u32>(head, 0), 0xA5A5_A500);
            assert_eq!(read::<u32>(head, 4), 0);
            assert_eq!(read::<*mut u8>(head, 8), head);
            assert_eq!(read::<*mut u8>(head, 12), head);
            assert_eq!(read::<u32>(head, 16), 0xA5A5_A5A5);
            *region = 0x1234_5678 as *mut u8;
            cost.write(0xABCD_EF01);
            cost.add(1).write(0x9876_5432);
            (*tree).count = s.new_count;
            s.new_tree = head as usize;
            if s.replacement_tree != 0 {
                (*tree).head = s.replacement_tree as *mut u8;
            }
            s.success
        })
    }
}

extern "C" fn swap(a: *mut u8, b: *mut Tree) {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Swap);
        assert_eq!(a as usize, s.tool + 0x214);
        if s.swap_replacement != 0 {
            unsafe {
                write(a, 0, s.swap_replacement as u32);
                write(a, 4, s.swap_count);
                (*b).count = 0x7654;
            }
        }
    });
}

extern "thiscall" fn erase(tree: *mut Tree, root: *mut u8) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Erase(root as usize));
        if s.erase_replacement != 0 {
            unsafe {
                (*tree).head = s.erase_replacement as *mut u8;
            }
        }
    });
    registers(0xEEEE_0000, 0xDDDD_0000)
}

struct World {
    object: Object,
    _vtable: Box<[usize; 18]>,
    _fakes: Vec<Fake>,
    spans: Vec<[u32; 3]>,
    cells: Vec<[u32; 2]>,
    extras: Vec<[u32; 5]>,
}

impl World {
    fn new(spans: Vec<[u32; 3]>, cells: Vec<[u32; 2]>, extras: Vec<[u32; 5]>) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                success: 1,
                ..State::default()
            }
        });
        let mut w = Self {
            object: Object::new(0x370),
            _vtable: Box::new([0; 18]),
            _fakes: vec![
                fake(0x0063_3220, lookup as *const () as usize),
                fake(0x007C_61A0, grow as *const () as usize),
                fake(0x0090_CF54, alloc as *const () as usize),
                fake(0x0090_CF63, free_fake as *const () as usize),
                fake(0x005E_5620, region_free as *const () as usize),
                fake(0x0061_F7C0, compute as *const () as usize),
                fake(0x0061_F5D0, swap as *const () as usize),
                fake(0x0046_76A0, erase as *const () as usize),
            ],
            spans,
            cells,
            extras,
        };
        w._vtable[17] = clear as *const () as usize;
        let p = w.object.ptr();
        unsafe {
            write(p, 0, w._vtable.as_ptr());
        }
        let tool = unsafe { HighlightTool::from_ptr(p) };
        tool.set_span_count(w.spans.len() as i32);
        tool.set_spans(w.spans.as_mut_ptr().cast());
        tool.set_cells(w.cells.as_mut_ptr().cast());
        tool.set_intersections(w.extras.as_mut_ptr().cast());
        tool.set_intersections_end(w.extras.as_mut_ptr().wrapping_add(w.extras.len()).cast());
        tool.set_filter(0xCAFE_F00D);
        STATE.with(|s| s.borrow_mut().tool = p as usize);
        w
    }
    fn tool(&mut self) -> HighlightTool {
        unsafe { HighlightTool::from_ptr(self.object.ptr()) }
    }
    fn run(&mut self, arg: u32) -> (u32, u32) {
        unsafe {
            call_thiscall_with_eax(
                fc4p_006299a0 as *const () as usize,
                self.object.ptr(),
                &[arg],
                0xBBBB_AAAA,
            )
        }
    }
}

fn calls() -> Vec<Call> {
    STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
}

#[test]
fn no_valid_spans_only_clears_highlights() {
    let mut w = World::new(vec![], vec![], vec![[1, 2, 0, 0, 0]]);
    assert_eq!(w.run(99), (0, 0x5566_7788));
    assert_eq!(calls(), [Call::Clear]);
    assert_eq!(w.tool().region(), null_mut());
}

#[unsafe(naked)]
unsafe extern "C" fn clear_preserving_edx() {
    core::arch::naked_asm!("mov eax, 0x12345678", "ret");
}

#[test]
fn empty_pass_supplies_entry_edx_to_a_clear_callback_that_preserves_it() {
    let mut w = World::new(vec![], vec![], vec![]);
    w._vtable[17] = clear_preserving_edx as *const () as usize;
    let arguments = [17u32];
    let mut c = RawCall::new(
        Conv::Thiscall,
        fc4p_006299a0 as *const () as usize,
        w.object.ptr() as u32,
        &arguments,
    );
    c.edx = 0xAABB_CCDD;
    unsafe {
        call_raw(&mut c);
    }
    assert_eq!((c.eax, c.edx), (0, 0xAABB_CCDD));
}

#[test]
fn ordered_candidates_keep_duplicates_and_filter_only_byte_56() {
    let mut w = World::new(
        vec![[0, 2, 0], [0, 1, 3], [4, 3, 0], [0, 0, 2]],
        vec![[7, 8], [9, 10], [11, 12]],
        vec![[7, 8, 1, 2, 3], [9, 10, 4, 5, 6], [13, 14, 7, 8, 9]],
    );
    let mut blocked = [0u8; 0x58];
    blocked[0x56] = 0x80;
    let mut allowed = [0u8; 0x58];
    allowed[0x51] = 1;
    STATE.with(|s| {
        s.borrow_mut().cells = vec![
            (0xA0009, blocked.as_mut_ptr() as usize),
            (0xC000B, allowed.as_mut_ptr() as usize),
        ]
    });
    assert_eq!(w.run(0xF000_0002).0, 0xF000_0002);
    let log = calls();
    assert_eq!(
        log.iter()
            .filter_map(|c| if let Call::Cell(k) = c { Some(*k) } else { None })
            .collect::<Vec<_>>(),
        [0x80007, 0xA0009, 0xC000B, 0x80007, 0x80007, 0xA0009, 0xE000D]
    );
    assert!(log.contains(&Call::Compute(
        vec![[7, 8], [11, 12], [7, 8], [7, 8], [13, 14]],
        0xCAFE_F00D
    )));
    assert_eq!(
        log.iter()
            .filter_map(|c| if let Call::Grow(p) = c { Some(*p) } else { None })
            .collect::<Vec<_>>(),
        [[7, 8], [11, 12], [7, 8]]
    );
    assert_eq!(
        log.iter()
            .filter_map(|c| if let Call::Allocate(n) = c { Some(*n) } else { None })
            .collect::<Vec<_>>(),
        [8, 16, 32, 64, 20]
    );
    let tool = w.tool();
    assert_eq!(tool.region() as usize, 0x1234_5678);
    assert_eq!(unsafe { read::<u64>(tool.as_ptr(), 0x220) }, 0x9876_5432_ABCD_EF01);
    let new_tree = STATE.with(|s| s.borrow().new_tree);
    assert_eq!(tool.tree() as usize, new_tree);
    assert_eq!(tool.tree_count(), 0);
}

#[test]
fn negative_count_still_collects_extra_records_and_wraps_failure_result() {
    let mut w = World::new(vec![], vec![], vec![[0xFFFF_FFFF, 2, 0, 0, 0]]);
    w.tool().set_span_count(-1);
    STATE.with(|s| s.borrow_mut().success = 0xABCD_0000);
    let result = w.run(0);
    assert_eq!(result.0, u32::MAX);
    let log = calls();
    assert_eq!(log[0], Call::Cell(0x1_FFFF));
    assert!(log.contains(&Call::Compute(vec![[u32::MAX, 2]], 0xCAFE_F00D)));
    let Some(Call::Free(last)) = log.last() else {
        panic!("{log:?}")
    };
    assert_eq!(result.1, 0xED00_0000 | *last as u32 & 0x00FF_FFFF);
}

#[test]
fn growth_receives_callers_word_and_span_bounds_are_reloaded() {
    let mut w = World::new(vec![[0, 2, 0], [1, 2, 0]], vec![[1, 2], [3, 4], [5, 6]], vec![]);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.change_argument = true;
        s.mutate_span = w.spans.as_mut_ptr() as usize;
    });
    assert_eq!(w.run(123).0, 0x8765_4321);
    assert!(calls().contains(&Call::Compute(vec![[1, 2]], 0xCAFE_F00D)));
}

#[test]
fn extra_coordinates_are_read_after_lookup() {
    let mut w = World::new(vec![[1, 0, 0]], vec![], vec![[1, 2, 0, 0, 0]]);
    STATE.with(|s| s.borrow_mut().mutate_record = w.extras.as_mut_ptr() as usize);
    assert_eq!(w.run(17).0, 17);
    let log = calls();
    assert_eq!(log[0], Call::Cell(0x20001));
    assert!(log.contains(&Call::Compute(vec![[0xCAFE, 0xFACE]], 0xCAFE_F00D)));
}

#[test]
fn destroys_old_region_then_swaps_and_erases_old_tree_in_order() {
    let mut w = World::new(vec![[1, 0, 0]], vec![], vec![]);
    let mut region = [0u32; 9];
    let mut columns = [0x1111u32, 0x2222];
    region[5] = 2;
    region[8] = columns.as_mut_ptr() as u32;
    let mut old_head = [0u32, 0xABCD, 0, 0, 0];
    let mut new_head = [0xCCCC_CCCCu32; 5];
    w.tool().set_region(region.as_mut_ptr().cast());
    w.tool().set_tree(old_head.as_mut_ptr().cast());
    w.tool().set_tree_count(4);
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.erase_replacement = new_head.as_mut_ptr() as usize;
        s.new_count = 6;
    });
    assert_eq!(w.run(11).0, 11);
    let log = calls();
    assert_eq!(
        log[0..3],
        [
            Call::RegionFree(0x1111),
            Call::RegionFree(0x2222),
            Call::RegionFree(columns.as_ptr() as usize)
        ]
    );
    assert_eq!(log[3], Call::RegionFree(region.as_ptr() as usize));
    assert_eq!(region[4], 0x00A8_6364);
    assert_eq!(log[4], Call::Allocate(20));
    assert!(matches!(&log[5], Call::Compute(c, _) if c.is_empty()));
    assert_eq!(
        log[6..],
        [Call::Swap, Call::Erase(0xABCD), Call::Free(new_head.as_ptr() as usize)]
    );
    assert_eq!(old_head, [0, 0xABCD, 0, 0, 0]);
    assert_eq!(
        new_head,
        [
            0xCCCC_CCCC,
            0,
            new_head.as_ptr() as u32,
            new_head.as_ptr() as u32,
            0xCCCC_CCCC
        ]
    );
    assert_eq!(w.tool().tree_count(), 6);
}

#[test]
fn swap_callback_changes_are_observed_and_edx_keeps_new_count_without_frees() {
    let mut w = World::new(vec![[1, 0, 0]], vec![], vec![]);
    let mut replacement = [0, 0x1212, 0, 0, 0u32];
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.swap_replacement = replacement.as_mut_ptr() as usize;
        s.swap_count = 2;
    });
    assert_eq!(w.run(12).0, 12);
    let log = calls();
    assert!(log.contains(&Call::Erase(0x1212)));
    assert_eq!(w.tool().tree_count(), 0x7654);

    let mut w = World::new(vec![[1, 0, 0]], vec![], vec![]);
    STATE.with(|s| s.borrow_mut().new_count = 0x1122_3344);
    assert_eq!(w.run(0xFFFF_FFFF), (0xFFFF_FFFF, 0x1122_3344));
    assert!(matches!(calls().last(), Some(Call::Swap)));
}

#[test]
fn empty_region_columns_and_absent_column_storage_are_destroyed() {
    for with_storage in [false, true] {
        let mut w = World::new(vec![[u32::MAX, 0, 0]], vec![], vec![]);
        let mut region = [0u32; 9];
        let mut columns = [0x1111u32];
        if with_storage {
            region[8] = columns.as_mut_ptr() as u32;
        }
        w.tool().set_region(region.as_mut_ptr().cast());
        w.run(0);
        let log = calls();
        let deletes: Vec<_> = log.iter().filter(|c| matches!(c, Call::RegionFree(..))).collect();
        if with_storage {
            assert_eq!(
                deletes,
                [
                    &Call::RegionFree(columns.as_ptr() as usize),
                    &Call::RegionFree(region.as_ptr() as usize)
                ]
            );
        } else {
            assert_eq!(deletes, [&Call::RegionFree(region.as_ptr() as usize)]);
        }
        assert!(log.contains(&Call::Compute(vec![], 0xCAFE_F00D)));
    }
}

#[test]
fn region_columns_and_count_are_reloaded_after_free() {
    let mut w = World::new(vec![[1, 0, 0]], vec![], vec![]);
    let mut region = [0u32; 9];
    let mut columns = [0x1111u32, 0x2222];
    let mut other_columns = [0x3333u32];
    region[5] = 2;
    region[8] = columns.as_mut_ptr() as u32;
    w.tool().set_region(region.as_mut_ptr().cast());
    STATE.with(|s| s.borrow_mut().replacement_region = other_columns.as_mut_ptr() as usize);
    w.run(1);
    let log = calls();
    assert_eq!(
        log[0..3],
        [
            Call::RegionFree(0x1111),
            Call::RegionFree(other_columns.as_ptr() as usize),
            Call::RegionFree(region.as_ptr() as usize)
        ]
    );
}
