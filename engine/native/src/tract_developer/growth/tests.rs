// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Aggregation ownership, cache removal, replacement geometry, and terrain notification.

use super::*;
use crate::testing::{fake, fake_data, Fake, Object};
use crate::tract_developer::testing::{run, snapshot};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Add(u32),
    Release(u32),
    Erase(u32),
    Allocate(u32),
    Free(u32),
    Demolish(u32),
    Notify(u32),
    Purpose,
    Size,
    Create(Vec<u32>),
    Terrain([u32; 4]),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    memory: Vec<Box<[u32]>>,
    purpose: u32,
    size: [u32; 2],
    references: [i32; 2],
    result: u32,
    output: u32,
    terrain_clears: bool,
    cache: Vec<u32>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
extern "thiscall" fn add(this: u32) -> u64 {
    let id = unsafe { word(this, 4) };
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Add(id));
        s.references[id as usize] += 1;
    });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn release(this: u32) -> u64 {
    let id = unsafe { word(this, 4) };
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Release(id));
        s.references[id as usize] -= 1;
    });
    0xBADC_0FFE_1234_5678
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        let mut memory = vec![0xCCCC_CCCCu32; bytes as usize / 4].into_boxed_slice();
        let p = memory.as_mut_ptr() as u32;
        s.memory.push(memory);
        0xBADC_0FFE_0000_0000 | u64::from(p)
    })
}
extern "C" fn free(p: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Free(p)));
    0xBADC_0FFE_1234_5678
}
extern "C" fn erase(node: u32, root: u32, first: u32, last: u32) -> u64 {
    assert_eq!(first, root + 4);
    assert_eq!(last, root + 8);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Erase(node));
        s.cache.retain(|&p| p != node);
        unsafe {
            store(root, 0, s.cache.first().copied().unwrap_or(0));
            store(first, 0, s.cache.first().copied().unwrap_or(root - 4));
            store(last, 0, s.cache.last().copied().unwrap_or(root - 4));
        }
    });
    0xBADC_0FFE_0000_0000 | u64::from(node)
}
extern "thiscall" fn demolish(
    _this: u32,
    flag: u32,
    list: u32,
    a: u32,
    mask: u32,
    b: u32,
    c: u32,
    d: u32,
    e: u32,
) -> u64 {
    assert_eq!([flag, a, mask, b, c, d, e], [1, 0, 0x800, 0, 0, 0, 0]);
    unsafe {
        let header = word(list, 0);
        let node = word(header, 0);
        assert_ne!(node, header);
        assert_eq!(word(header, 4), node);
        assert_eq!(word(header, 8), 0xCCCC_CCCC);
        assert_eq!([word(node, 0), word(node, 4)], [header, header]);
        let lot = word(node, 8);
        STATE.with(|s| s.borrow_mut().calls.push(Call::Demolish(word(lot, 4))));
    }
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn notify(_this: u32, lot: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Notify(unsafe { word(lot, 4) })));
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn purpose(_this: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Purpose);
        0xBADC_0FFE_0000_0000 | u64::from(s.purpose)
    })
}
extern "thiscall" fn size(_this: u32, width: u32, depth: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Size);
        unsafe {
            store(width, 0, s.size[0]);
            store(depth, 0, s.size[1]);
        }
    });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn create(
    _this: u32,
    x: u32,
    z: u32,
    width: u32,
    depth: u32,
    facing: u32,
    output: u32,
    a: u32,
    b: u32,
    c: u32,
) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Create(vec![x, z, width, depth, facing, a, b, c]));
        unsafe {
            assert_eq!(word(output, 0), 0);
            store(output, 0, s.result);
        }
        s.output = output;
    });
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn terrain(_this: u32, a: u32, b: u32, rectangle: u32, c: u32) -> u64 {
    assert_eq!([a, b, c], [0; 3]);
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Terrain(unsafe {
            [
                word(rectangle, 0),
                word(rectangle, 4),
                word(rectangle, 8),
                word(rectangle, 12),
            ]
        }));
        if s.terrain_clears {
            unsafe {
                store(s.output, 0, 0);
            }
        }
    });
    0x1234_5678_8765_4321
}

struct World {
    td: Object,
    candidate: Object,
    nodes: Box<[[u32; 5]; 3]>,
    cache: Box<[[u32; 6]; 3]>,
    lots: Box<[[u32; 2]; 2]>,
    objects: Box<[u32; 5]>,
    tables: [Box<[usize; 70]>; 6],
    demolition: Box<u32>,
    terrain: Box<u32>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new(count: usize, cached: bool) -> Self {
        STATE.with(|s| {
            *s.borrow_mut() = State {
                purpose: 2,
                result: 0xABCD_EF01,
                ..State::default()
            }
        });
        let mut w = Self {
            td: Object::new(0x180),
            candidate: Object::new(0xA4),
            nodes: Box::new([[0; 5]; 3]),
            cache: Box::new([[0; 6]; 3]),
            lots: Box::new([[0; 2]; 2]),
            objects: Box::new([0; 5]),
            tables: core::array::from_fn(|_| Box::new([0; 70])),
            demolition: Box::new(0),
            terrain: Box::new(0),
            _fakes: Vec::new(),
        };
        for (table, slot, target) in [
            (0, 0x1C, purpose as *const () as usize),
            (1, 0x60, notify as *const () as usize),
            (1, 0x58, create as *const () as usize),
            (2, 0x1C, size as *const () as usize),
            (3, 0x1C, demolish as *const () as usize),
            (4, 0x110, terrain as *const () as usize),
            (5, 4, add as *const () as usize),
            (5, 8, release as *const () as usize),
        ] {
            w.tables[table][slot / 4] = target;
        }
        for i in 0..5 {
            w.objects[i] = w.tables[i].as_ptr() as u32;
        }
        for i in 0..2 {
            w.lots[i] = [w.tables[5].as_ptr() as u32, i as u32];
        }
        let header = w.nodes[0].as_ptr() as u32;
        w.nodes[0] = [
            0,
            if count == 0 { 0 } else { w.nodes[1].as_ptr() as u32 },
            header,
            header,
            0,
        ];
        if count > 0 {
            w.nodes[0][2] = w.nodes[1].as_ptr() as u32;
            w.nodes[0][3] = w.nodes[count].as_ptr() as u32;
            for i in 1..=count {
                w.nodes[i] = [
                    1,
                    if i == 1 { header } else { w.nodes[i - 1].as_ptr() as u32 },
                    0,
                    if i == count { 0 } else { w.nodes[i + 1].as_ptr() as u32 },
                    w.lots[i - 1].as_ptr() as u32,
                ];
            }
        }
        let ch = w.cache[0].as_ptr() as u32;
        w.cache[0] = [0, 0, ch, ch, 0, 0];
        if cached && count > 0 {
            w.cache[0][1] = w.cache[1].as_ptr() as u32;
            for i in 1..=count {
                w.cache[i] = [
                    1,
                    ch,
                    0,
                    if i == count { 0 } else { w.cache[i + 1].as_ptr() as u32 },
                    w.lots[i - 1].as_ptr() as u32,
                    27,
                ];
            }
            STATE.with(|s| s.borrow_mut().cache = (1..=count).map(|i| w.cache[i].as_ptr() as u32).collect());
        }
        let td = unsafe { TractDeveloper::from_ptr(w.td.ptr()) };
        td.set_developer(w.objects.as_ptr() as u32);
        td.set_chosen_configuration(w.objects.as_ptr().wrapping_add(2) as u32);
        td.set_built(i32::MIN);
        td.set_suitability_header(ch);
        td.set_suitability_count(if cached { count as u32 } else { 0 });
        unsafe {
            store(td.as_ptr() as u32, 0x2C, w.objects.as_ptr().wrapping_add(1) as u32);
        }
        let c = unsafe { Candidate::from_ptr(w.candidate.ptr()) };
        c.set_lot_header(header);
        c.set_occupied(1);
        c.set_facing(3);
        c.set_min_x(10);
        c.set_min_z(20);
        c.set_max_x(12);
        c.set_max_z(24);
        *w.demolition = w.objects.as_ptr().wrapping_add(3) as u32;
        *w.terrain = w.objects.as_ptr().wrapping_add(4) as u32;
        for (address, target) in [
            (0x0090_CF54, allocate as *const () as usize),
            (0x0090_CF63, free as *const () as usize),
            (0x0040_DAF0, erase as *const () as usize),
        ] {
            w._fakes.push(fake(address, target));
        }
        w._fakes
            .push(fake_data(0x00B4_3D00, (&mut *w.demolition as *mut u32).cast()));
        w._fakes
            .push(fake_data(0x00B4_3CF4, (&mut *w.terrain as *mut u32).cast()));
        w
    }
    fn run(&mut self) -> u32 {
        let mut output = 0x7777_7777u32;
        let before = snapshot(self.candidate.ptr(), 0xA4);
        let result = run(
            0x0070_68E0,
            Conv::Thiscall,
            self.td.ptr() as u32,
            &[self.candidate.ptr() as u32, &mut output as *mut u32 as u32],
        );
        assert_eq!(result.0, u32::from(output != 0));
        assert_eq!(result.1, output);
        assert_eq!(snapshot(self.candidate.ptr(), 0xA4), before);
        assert_eq!(unsafe { TractDeveloper::from_ptr(self.td.ptr()) }.built(), i32::MAX);
        output
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn aggregation_demolishes_in_tree_order_balances_references_and_removes_cached_entries() {
    let mut w = World::new(2, true);
    assert_eq!(w.run(), 0xABCD_EF01);
    let pointers = STATE.with(|s| s.borrow().memory.iter().map(|m| m.as_ptr() as u32).collect::<Vec<_>>());
    let mut expected = Vec::new();
    for id in 0..2u32 {
        expected.extend([
            Call::Add(id),
            Call::Erase(w.cache[id as usize + 1].as_ptr() as u32),
            Call::Free(w.cache[id as usize + 1].as_ptr() as u32),
            Call::Allocate(12),
            Call::Add(id),
            Call::Allocate(12),
            Call::Add(id),
            Call::Release(id),
            Call::Demolish(id),
            Call::Notify(id),
            Call::Release(id),
            Call::Release(id),
            Call::Free(pointers[id as usize * 2 + 1]),
            Call::Free(pointers[id as usize * 2]),
        ]);
    }
    expected.extend([
        Call::Purpose,
        Call::Create(vec![10, 20, 3, 5, 3, 0, 0, 0]),
        Call::Terrain([10, 20, 12, 24]),
    ]);
    assert_eq!(w.calls(), expected);
    assert_eq!(STATE.with(|s| s.borrow().references), [0; 2]);
    assert_eq!(unsafe { TractDeveloper::from_ptr(w.td.ptr()) }.suitability_count(), 0);
}

#[test]
fn empty_aggregation_industrial_square_facing_and_terrain_result_side_effect() {
    for (purpose, size, auto_facing) in [
        (4, [2, 2], false),
        (5, [2, 2], true),
        (8, [2, 2], true),
        (9, [2, 2], false),
        (6, [2, 3], false),
    ] {
        let mut w = World::new(0, false);
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.purpose = purpose;
            s.size = size;
            s.terrain_clears = true;
        });
        assert_eq!(w.run(), 0);
        let mut expected = vec![Call::Purpose];
        if (5..=8).contains(&purpose) {
            expected.push(Call::Size);
        }
        expected.extend([
            Call::Create(vec![10, 20, 3, 5, if auto_facing { u32::MAX } else { 3 }, 0, 0, 0]),
            Call::Terrain([10, 20, 12, 24]),
        ]);
        assert_eq!(w.calls(), expected);
    }
}

#[test]
fn uncached_lot_failed_creation_and_absent_terrain() {
    let mut w = World::new(1, false);
    *w.terrain = 0;
    STATE.with(|s| s.borrow_mut().result = 0);
    assert_eq!(w.run(), 0);
    let calls = w.calls();
    assert!(!calls.iter().any(|c| matches!(c, Call::Erase(_) | Call::Terrain(_))));
    assert_eq!(calls.last(), Some(&Call::Create(vec![10, 20, 3, 5, 3, 0, 0, 0])));
    assert_eq!(STATE.with(|s| s.borrow().references), [0; 2]);
}
