// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Removal across valid red-black shapes, plus exact rotation order and byte-preservation checks.

use super::*;
use crate::testing::{fake, Fake};
use crate::tract_developer::testing::run;
use std::cell::RefCell;

thread_local! { static CALLS: RefCell<Vec<(bool, u32, u32)>> = const { RefCell::new(Vec::new()) }; }
unsafe extern "stdcall" fn record_left(args: *const u32) {
    unsafe {
        CALLS.with(|c| c.borrow_mut().push((true, *args, *args.add(1))));
    }
}
unsafe extern "stdcall" fn record_right(args: *const u32) {
    unsafe {
        CALLS.with(|c| c.borrow_mut().push((false, *args, *args.add(1))));
    }
}
#[unsafe(naked)]
unsafe extern "C" fn left() {
    core::arch::naked_asm!(
        "push eax", "push ecx", "push edx", "lea eax, [esp + 16]", "push eax", "call {record}",
        "pop edx", "pop ecx", "pop eax", "jmp {rotation}",
        record = sym record_left, rotation = sym super::super::trees::fc4p_0040d540,
    );
}
#[unsafe(naked)]
unsafe extern "C" fn right() {
    core::arch::naked_asm!(
        "push eax", "push ecx", "push edx", "lea eax, [esp + 16]", "push eax", "call {record}",
        "pop edx", "pop ecx", "pop eax", "jmp {rotation}",
        record = sym record_right, rotation = sym super::super::trees::fc4p_0040d590,
    );
}

#[derive(Clone, Debug)]
enum Tree {
    Empty,
    Node(u8, Box<Tree>, Box<Tree>),
}
use Tree::{Empty, Node as Branch};
fn node(color: u8, a: Tree, b: Tree) -> Tree {
    Branch(color, Box::new(a), Box::new(b))
}
fn black(a: Tree, b: Tree) -> Tree {
    node(1, a, b)
}
fn red(a: Tree, b: Tree) -> Tree {
    node(0, a, b)
}
impl Tree {
    fn len(&self) -> usize {
        match self {
            Empty => 0,
            Branch(_, a, b) => 1 + a.len() + b.len(),
        }
    }
    fn color(&self) -> u8 {
        match self {
            Empty => 1,
            Branch(c, _, _) => *c,
        }
    }
    fn height(&self) -> usize {
        match self {
            Empty => 0,
            Branch(c, a, _) => a.height() + usize::from(*c == 1),
        }
    }
}
fn shapes(n: usize) -> Vec<Tree> {
    if n == 0 {
        return vec![Empty];
    }
    let mut result = Vec::new();
    for left_size in 0..n {
        for a in shapes(left_size) {
            for b in shapes(n - left_size - 1) {
                if a.height() == b.height() {
                    result.push(black(a.clone(), b.clone()));
                    if a.color() == 1 && b.color() == 1 {
                        result.push(red(a.clone(), b));
                    }
                }
            }
        }
    }
    result
}

struct World {
    memory: Box<[[u32; 6]]>,
    _fakes: [Fake; 2],
}
impl World {
    fn new(shape: &Tree) -> Self {
        CALLS.with(|c| c.borrow_mut().clear());
        let mut w = Self {
            memory: vec![[0; 6]; shape.len() + 1].into_boxed_slice(),
            _fakes: [
                fake(0x0040_D540, left as *const () as usize),
                fake(0x0040_D590, right as *const () as usize),
            ],
        };
        let header = w.pointer(0);
        let root = w.fill(shape, 0, header);
        w.memory[0] = [
            0xA5A5_A500,
            root,
            if root == 0 { header } else { w.pointer(1) },
            if root == 0 { header } else { w.pointer(shape.len()) },
            0xFEED_FACE,
            0xDEAD_BEEF,
        ];
        w
    }
    fn pointer(&self, i: usize) -> u32 {
        self.memory[i].as_ptr() as u32
    }
    fn index(&self, p: u32) -> usize {
        assert_ne!(p, 0);
        let difference = p.wrapping_sub(self.pointer(0));
        assert_eq!(difference % 24, 0);
        let i = difference as usize / 24;
        assert!(i < self.memory.len());
        i
    }
    fn fill(&mut self, shape: &Tree, base: usize, parent: u32) -> u32 {
        match shape {
            Empty => 0,
            Branch(c, a, b) => {
                let i = base + a.len() + 1;
                let p = self.pointer(i);
                let left = self.fill(a, base, p);
                let right = self.fill(b, i, p);
                self.memory[i] = [
                    0xA5A5_A500 | u32::from(*c),
                    parent,
                    left,
                    right,
                    i as u32,
                    0xDEAD_BEEF ^ i as u32,
                ];
                p
            }
        }
    }
    fn validate(&self, p: u32, parent: u32, keys: &mut Vec<usize>) -> usize {
        if p == 0 {
            return 0;
        }
        let i = self.index(p);
        assert_ne!(i, 0);
        let n = self.memory[i];
        assert_eq!(n[1], parent);
        assert!(n[0] as u8 <= 1);
        let left = self.validate(n[2], p, keys);
        keys.push(i);
        let right = self.validate(n[3], p, keys);
        assert_eq!(left, right, "black height at node {i}");
        if n[0] as u8 == 0 {
            for p in [n[2], n[3]] {
                if p != 0 {
                    assert_eq!(self.memory[self.index(p)][0] as u8, 1);
                }
            }
        }
        left + usize::from(n[0] as u8 == 1)
    }
    fn remove(&mut self, i: usize) -> (u32, u32, u32) {
        let h = self.pointer(0);
        let p = self.pointer(i);
        let before = self.memory.to_vec();
        let result = run(0x0040_DAF0, Conv::Cdecl, 0x1234_ABCD, &[p, h + 4, h + 8, h + 12]);
        assert_eq!(result.0, p);
        let root = self.memory[0][1];
        let mut keys = Vec::new();
        self.validate(root, h, &mut keys);
        let expected: Vec<_> = (1..self.memory.len()).filter(|&key| key != i).collect();
        assert_eq!(keys, expected);
        if root == 0 {
            assert_eq!(&self.memory[0][1..4], &[0, h, h]);
        } else {
            assert_eq!(self.memory[self.index(root)][0] as u8, 1);
            assert_eq!(self.memory[0][2], self.pointer(*expected.first().unwrap()));
            assert_eq!(self.memory[0][3], self.pointer(*expected.last().unwrap()));
        }
        for (index, (old, new)) in before.iter().zip(self.memory.iter()).enumerate() {
            assert_eq!(&old[4..], &new[4..], "payload {index}");
            assert_eq!(old[0] & !0xFF, new[0] & !0xFF, "padding {index}");
        }
        assert_eq!(&self.memory[i][1..], &before[i][1..], "removed node links and payload");
        let mut successor = before[i][3];
        let expected_color = if before[i][2] != 0 && successor != 0 {
            while before[self.index(successor)][2] != 0 {
                successor = before[self.index(successor)][2];
            }
            before[self.index(successor)][0] as u8
        } else {
            before[i][0] as u8
        };
        assert_eq!(self.memory[i][0] as u8, expected_color);
        result
    }
    fn calls(&self) -> Vec<(bool, usize)> {
        CALLS.with(|c| {
            c.borrow()
                .iter()
                .map(|&(left, node, root)| {
                    assert_eq!(root, self.pointer(0) + 4);
                    (left, self.index(node))
                })
                .collect()
        })
    }
}

#[test]
fn all_valid_small_shapes_keep_links_order_colors_and_padding_after_each_removal() {
    let mut cases = 0usize;
    for size in 1..=8 {
        for shape in shapes(size).into_iter().filter(|shape| shape.color() == 1) {
            for i in 1..=size {
                World::new(&shape).remove(i);
                cases += 1;
            }
        }
    }
    assert!(cases > 500, "{cases}");
}

#[test]
fn near_and_far_red_children_use_the_original_rotation_call_order() {
    let leaf = || black(Empty, Empty);
    for (shape, removed, expected) in [
        (black(leaf(), black(Empty, red(Empty, Empty))), 1, vec![(true, 2)]),
        (black(black(red(Empty, Empty), Empty), leaf()), 4, vec![(false, 3)]),
        (
            black(leaf(), black(red(Empty, Empty), Empty)),
            1,
            vec![(false, 4), (true, 2)],
        ),
        (
            black(black(Empty, red(Empty, Empty)), leaf()),
            4,
            vec![(true, 1), (false, 3)],
        ),
    ] {
        let mut w = World::new(&shape);
        w.remove(removed);
        assert_eq!(w.calls(), expected);
    }
}

#[test]
fn red_sibling_promotion_is_internal_and_black_deficits_can_propagate() {
    let leaf = || black(Empty, Empty);
    let shape = black(leaf(), red(leaf(), leaf()));
    let mut w = World::new(&shape);
    w.remove(1);
    assert!(w.calls().is_empty());
    assert_eq!(w.memory[0][1], w.pointer(4));
    assert_eq!(w.memory[2][0] as u8, 1);
    assert_eq!(w.memory[3][0] as u8, 0);
    let shape = black(red(leaf(), leaf()), leaf());
    let mut w = World::new(&shape);
    w.remove(5);
    assert!(w.calls().is_empty());
    assert_eq!(w.memory[0][1], w.pointer(2));
    let shape = black(black(leaf(), leaf()), black(leaf(), leaf()));
    let mut w = World::new(&shape);
    w.remove(1);
    assert!(w.calls().is_empty());
    assert_eq!(w.memory[3][0] as u8, 0);
    assert_eq!(w.memory[6][0] as u8, 0);
}

#[test]
fn single_root_and_red_leaf_keep_branch_specific_volatile_registers() {
    let mut w = World::new(&black(Empty, Empty));
    let h = w.pointer(0);
    assert_eq!(w.remove(1), (w.pointer(1), h + 4, h + 12));
    let mut w = World::new(&black(red(Empty, Empty), red(Empty, Empty)));
    let h = w.pointer(0);
    assert_eq!(w.remove(1), (w.pointer(1), w.pointer(2), h + 12));
}
