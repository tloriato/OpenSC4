// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Signed pair ordering, duplicate results, neighbor hints and payload insertion.

use super::*;
use crate::testing::{fake, Fake};
use crate::tract_developer::testing::run;
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call {
    Allocate(u32),
    Rebalance(u32, u32),
    Previous(u32),
    Next(u32),
    Compare([i32; 2], [i32; 2]),
    Attach(u32, u32, [u32; 3], u32),
    Unique([u32; 3]),
    Lower([i32; 2]),
    Hint(u32, [u32; 3]),
}
#[derive(Default)]
struct State {
    calls: Vec<Call>,
    nodes: Vec<u32>,
    header: u32,
    tree: u32,
    result: u32,
    allocation: Option<Box<[u32; 7]>>,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
unsafe fn pair(p: u32) -> [i32; 2] {
    unsafe { [word(p, 0) as i32, word(p, 4) as i32] }
}
unsafe fn value(p: u32) -> [u32; 3] {
    unsafe { [word(p, 0), word(p, 4), word(p, 8)] }
}
extern "C" fn allocate(bytes: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        s.calls.push(Call::Allocate(bytes));
        assert_eq!(bytes, 28);
        let mut memory = Box::new([0xCCCC_CCCCu32; 7]);
        let p = memory.as_mut_ptr() as u32;
        s.allocation = Some(memory);
        0xBADC_0FFE_0000_0000 | u64::from(p)
    })
}
extern "C" fn rebalance(node: u32, root: u32) -> u64 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Rebalance(node, root)));
    unsafe {
        *(node as *mut u8) = 1;
    }
    0xBADC_0FFE_1234_5678
}
extern "thiscall" fn previous(iterator: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let node = unsafe { word(iterator, 0) };
        s.calls.push(Call::Previous(node));
        let index = s.nodes.iter().position(|&p| p == node).unwrap();
        unsafe {
            store(iterator, 0, if index == 0 { s.header } else { s.nodes[index - 1] });
        }
        0xBADC_0FFE_1234_5678
    })
}
extern "thiscall" fn next(iterator: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        let node = unsafe { word(iterator, 0) };
        s.calls.push(Call::Next(node));
        let index = s.nodes.iter().position(|&p| p == node).unwrap();
        unsafe {
            store(iterator, 0, s.nodes.get(index + 1).copied().unwrap_or(s.header));
        }
        0xBADC_0FFE_1234_5678
    })
}
extern "thiscall" fn compare(this: u32, a: u32, b: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.tree + 8);
        let (a, b) = unsafe { (pair(a), pair(b)) };
        s.calls.push(Call::Compare(a, b));
        0xBADC_0FFE_1234_5600 | u64::from(a < b)
    })
}
extern "thiscall" fn attach(this: u32, output: u32, x: u32, parent: u32, v: u32, force: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.tree);
        s.calls.push(Call::Attach(x, parent, unsafe { value(v) }, force));
        unsafe {
            store(output, 0, s.result);
        }
        0xBADC_0FFE_0000_0000 | u64::from(output)
    })
}
extern "thiscall" fn unique(this: u32, output: u32, v: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.tree);
        s.calls.push(Call::Unique(unsafe { value(v) }));
        unsafe {
            store(output, 0, s.result);
            *((output + 4) as *mut u8) = 1;
        }
        0xBADC_0FFE_0000_0000 | u64::from(output)
    })
}
extern "thiscall" fn lower(this: u32, key: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.tree);
        let key = unsafe { pair(key) };
        s.calls.push(Call::Lower(key));
        let p = s
            .nodes
            .iter()
            .copied()
            .find(|&p| unsafe { pair(p + 0x10) } >= key)
            .unwrap_or(s.header);
        0xBADC_0FFE_0000_0000 | u64::from(p)
    })
}
extern "thiscall" fn hint(this: u32, output: u32, hint: u32, v: u32) -> u64 {
    STATE.with(|s| {
        let s = &mut *s.borrow_mut();
        assert_eq!(this, s.tree);
        s.calls.push(Call::Hint(hint, unsafe { value(v) }));
        unsafe {
            store(output, 0, s.result);
        }
        0xBADC_0FFE_0000_0000 | u64::from(output)
    })
}

struct World {
    nodes: Box<[[u32; 7]; 7]>,
    tree: Box<[u32; 3]>,
    _fakes: Vec<Fake>,
}
impl World {
    fn new() -> Self {
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut w = Self {
            nodes: Box::new([[0xCCCC_CCCC; 7]; 7]),
            tree: Box::new([0; 3]),
            _fakes: Vec::new(),
        };
        for i in 1..=5 {
            w.nodes[i] = [1, 0, 0, 0, i as u32 * 10, 0, 0x1000 + i as u32];
        }
        w.nodes[0] = [0, w.node(3), w.node(1), w.node(5), 0, 0, 0];
        w.nodes[3][1] = w.node(0);
        w.nodes[3][2] = w.node(1);
        w.nodes[3][3] = w.node(5);
        w.nodes[1][1] = w.node(3);
        w.nodes[1][3] = w.node(2);
        w.nodes[2][1] = w.node(1);
        w.nodes[5][1] = w.node(3);
        w.nodes[5][2] = w.node(4);
        w.nodes[4][1] = w.node(5);
        *w.tree = [w.node(0), 5, 0x1234_5678];
        STATE.with(|s| {
            let s = &mut *s.borrow_mut();
            s.nodes = (1..=5).map(|i| w.node(i)).collect();
            s.header = w.node(0);
            s.result = w.node(6);
            s.tree = w.tree.as_ptr() as u32;
        });
        for (address, function) in [
            (0x0090_CF54, allocate as *const () as usize),
            (0x0040_D5E0, rebalance as *const () as usize),
            (0x0068_09E0, previous as *const () as usize),
            (0x005D_17C0, next as *const () as usize),
            (0x0063_37B0, compare as *const () as usize),
            (0x0063_C070, lower as *const () as usize),
        ] {
            w._fakes.push(fake(address, function));
        }
        w
    }
    fn node(&self, i: usize) -> u32 {
        self.nodes[i].as_ptr() as u32
    }
    fn fake_attach(&mut self) {
        self._fakes.push(fake(0x0070_5B80, attach as *const () as usize));
    }
    fn fake_unique(&mut self) {
        self._fakes.push(fake(0x0070_5C60, unique as *const () as usize));
    }
    fn calls(&self) -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }
}

#[test]
fn node_attachment_keeps_padding_and_calls_rebalance_before_count_increment() {
    for (parent_index, x, force, key, left) in [
        (0, 0, 0, 1, true),
        (1, 0, 0, 9, true),
        (5, 0, 0, 51, false),
        (3, 0, 1, 1, false),
        (3, 123, 0, 99, true),
    ] {
        let mut w = World::new();
        let parent = w.node(parent_index);
        let tree = w.tree.as_mut_ptr() as u32;
        let mut v = [key, 0xFFFF_0000, 0xDEAD_BEEF];
        let mut output = [0xABCD_1234u32; 2];
        let result = run(
            0x0070_5B80,
            Conv::Thiscall,
            tree,
            &[output.as_mut_ptr() as u32, x, parent, v.as_mut_ptr() as u32, force],
        );
        assert_eq!(result.0, output.as_ptr() as u32);
        let p = output[0];
        assert_eq!(output[1], 0xABCD_1234);
        assert_eq!(
            unsafe { std::slice::from_raw_parts(p as *const u32, 7) },
            [0xCCCC_CC01, parent, 0, 0, key, 0xFFFF_0000, 0xDEAD_BEEF]
        );
        assert_eq!(w.nodes[parent_index][if left { 2 } else { 3 }], p);
        assert_eq!(w.tree[1], 6);
        if parent_index == 0 {
            assert_eq!(w.nodes[0][1], p);
            assert_eq!(w.nodes[0][3], p);
        }
        if parent_index == 1 {
            assert_eq!(w.nodes[0][2], p);
        }
        if parent_index == 5 {
            assert_eq!(w.nodes[0][3], p);
        }
        assert_eq!(w.calls(), [Call::Allocate(28), Call::Rebalance(p, w.node(0) + 4)]);
    }
}

#[test]
fn unique_insertion_preserves_duplicate_payloads_and_result_padding() {
    for key in [
        [5i32, 0],
        [10, -1],
        [10, 0],
        [10, 1],
        [20, 0],
        [25, 0],
        [30, 0],
        [40, 0],
        [50, 0],
        [55, 0],
        [i32::MIN, i32::MAX],
        [i32::MAX, i32::MIN],
    ] {
        let mut w = World::new();
        w.fake_attach();
        let mut v = [key[0] as u32, key[1] as u32, 0xDEAD_BEEF];
        let mut output = [0xA5u8; 8];
        let before_nodes = *w.nodes;
        let before_tree = *w.tree;
        let existing = (1..=5).find(|&i| [w.nodes[i][4] as i32, w.nodes[i][5] as i32] == key);
        let result = run(
            0x0070_5C60,
            Conv::Thiscall,
            w.tree.as_mut_ptr() as u32,
            &[output.as_mut_ptr() as u32, v.as_mut_ptr() as u32],
        );
        assert_eq!(result.0, output.as_ptr() as u32);
        assert_eq!(
            u32::from_le_bytes(output[..4].try_into().unwrap()),
            w.node(existing.unwrap_or(6))
        );
        assert_eq!(&output[4..], &[u8::from(existing.is_none()), 0xA5, 0xA5, 0xA5]);
        assert_eq!(*w.nodes, before_nodes);
        assert_eq!(*w.tree, before_tree);
        let calls = w.calls();
        assert_eq!(
            calls.iter().filter(|c| matches!(c, Call::Attach(..))).count(),
            usize::from(existing.is_none())
        );
        if let Some(Call::Attach(x, parent, value, force)) = calls.last() {
            assert_eq!((*x, *value, *force), (0, v, 0));
            let wanted = if key < [10, 0] {
                1
            } else if key < [30, 0] {
                2
            } else if key < [50, 0] {
                4
            } else {
                5
            };
            assert_eq!(*parent, w.node(wanted));
        }
    }
}

#[test]
fn empty_unique_tree_inserts_under_header() {
    let mut w = World::new();
    w.fake_attach();
    let h = w.node(0);
    w.nodes[0] = [0, 0, h, h, 0, 0, 0];
    w.tree[1] = 0;
    let mut v = [0u32, 1, 2];
    let mut output = [0xA5u8; 8];
    run(
        0x0070_5C60,
        Conv::Thiscall,
        w.tree.as_mut_ptr() as u32,
        &[output.as_mut_ptr() as u32, v.as_mut_ptr() as u32],
    );
    assert_eq!(w.calls(), [Call::Attach(0, h, v, 0)]);
}

#[test]
fn hinted_insertion_uses_neighbors_and_falls_back_in_original_call_order() {
    for (hint_index, key, expected_kind) in [
        (1, 5, 0),
        (1, 10, 1),
        (1, 15, 2),
        (1, 25, 3),
        (0, 55, 4),
        (0, 20, 5),
        (3, 25, 6),
        (4, 35, 7),
        (3, 15, 8),
        (3, 30, 9),
        (3, 35, 10),
        (3, 40, 11),
        (5, 55, 12),
    ] {
        let mut w = World::new();
        w.fake_attach();
        w.fake_unique();
        let h = w.node(hint_index);
        let n = |i| w.node(i);
        let v = [key, 0, 0xDEAD_BEEF];
        let before = *w.nodes;
        let expected = match expected_kind {
            0 => vec![Call::Attach(n(1), n(1), v, 0)],
            1 => vec![],
            2 => vec![
                Call::Next(n(1)),
                Call::Compare([15, 0], [20, 0]),
                Call::Attach(n(2), n(2), v, 0),
            ],
            3 => vec![Call::Next(n(1)), Call::Compare([25, 0], [20, 0]), Call::Unique(v)],
            4 => vec![Call::Attach(0, n(5), v, n(0))],
            5 => vec![Call::Unique(v)],
            6 => vec![Call::Previous(n(3)), Call::Attach(0, n(2), v, n(2))],
            7 => vec![Call::Previous(n(4)), Call::Attach(n(4), n(4), v, 0)],
            8 => vec![Call::Previous(n(3)), Call::Next(n(3)), Call::Unique(v)],
            9 => vec![Call::Previous(n(3)), Call::Next(n(3)), Call::Compare([30, 0], [30, 0])],
            10 => vec![
                Call::Previous(n(3)),
                Call::Next(n(3)),
                Call::Compare([30, 0], [35, 0]),
                Call::Compare([35, 0], [40, 0]),
                Call::Attach(n(4), n(4), v, 0),
            ],
            11 => vec![
                Call::Previous(n(3)),
                Call::Next(n(3)),
                Call::Compare([30, 0], [40, 0]),
                Call::Compare([40, 0], [40, 0]),
                Call::Unique(v),
            ],
            _ => vec![
                Call::Previous(n(5)),
                Call::Next(n(5)),
                Call::Compare([50, 0], [55, 0]),
                Call::Attach(0, n(5), v, n(5)),
            ],
        };
        let mut output = [0xA5u32; 2];
        let result = run(
            0x0070_6040,
            Conv::Thiscall,
            w.tree.as_mut_ptr() as u32,
            &[output.as_mut_ptr() as u32, h, v.as_ptr() as u32],
        );
        assert_eq!(result.0, output.as_ptr() as u32);
        assert_eq!(
            output,
            [
                if expected_kind == 1 || expected_kind == 9 {
                    h
                } else {
                    w.node(6)
                },
                0xA5
            ]
        );
        assert_eq!(w.calls(), expected, "hint {hint_index}, key {key}");
        assert_eq!(*w.nodes, before);
    }
}

#[test]
fn hinted_empty_tree_and_clear_right_link_take_direct_paths() {
    let mut w = World::new();
    w.fake_attach();
    w.fake_unique();
    let h = w.node(0);
    w.nodes[0] = [0, 0, h, h, 0, 0, 0];
    w.tree[1] = 0;
    let v = [5u32, 0, 1];
    let mut output = 0u32;
    run(
        0x0070_6040,
        Conv::Thiscall,
        w.tree.as_mut_ptr() as u32,
        &[&mut output as *mut u32 as u32, h, v.as_ptr() as u32],
    );
    assert_eq!(w.calls(), [Call::Unique(v)]);
    let mut w = World::new();
    w.fake_attach();
    w.fake_unique();
    let p = w.node(1);
    w.nodes[1][3] = 0;
    let v = [15u32, 0, 1];
    run(
        0x0070_6040,
        Conv::Thiscall,
        w.tree.as_mut_ptr() as u32,
        &[&mut output as *mut u32 as u32, p, v.as_ptr() as u32],
    );
    assert_eq!(
        w.calls(),
        [Call::Next(p), Call::Compare([15, 0], [20, 0]), Call::Attach(0, p, v, p)]
    );
}

#[test]
fn payload_lookup_keeps_existing_value_or_inserts_zero_with_lower_bound_hint() {
    for key in [
        [10i32, 0],
        [10, -1],
        [10, 1],
        [25, 0],
        [50, 0],
        [55, 0],
        [i32::MIN, i32::MAX],
    ] {
        let mut w = World::new();
        w._fakes.push(fake(0x0070_6040, hint as *const () as usize));
        let mut key = key;
        let existing = (1..=5).find(|&i| unsafe { pair(w.node(i) + 0x10) } == key);
        let bound = (1..=5).find(|&i| unsafe { pair(w.node(i) + 0x10) } >= key).unwrap_or(0);
        let before = *w.nodes;
        let result = run(
            0x0070_74D0,
            Conv::Thiscall,
            w.tree.as_mut_ptr() as u32,
            &[key.as_mut_ptr() as u32],
        );
        assert_eq!(result.0, w.node(existing.unwrap_or(6)) + 0x18);
        let mut calls = vec![Call::Lower(key)];
        if existing.is_none() {
            calls.push(Call::Hint(w.node(bound), [key[0] as u32, key[1] as u32, 0]));
        }
        assert_eq!(w.calls(), calls);
        assert_eq!(*w.nodes, before);
    }
}
