// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

use super::*;
use crate::testing::{call_thiscall_with_eax, fake, fake_data, Object};
use std::cell::RefCell;

#[derive(Debug, PartialEq)]
enum Event {
    Free(usize),
    Recurse(usize, usize),
    Predicate(usize),
    Find(usize, u32),
}
#[derive(Default)]
struct State {
    events: Vec<Event>,
    hit: usize,
    replacement: usize,
    root: usize,
    found: usize,
}
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn record(event: Event) {
    STATE.with(|s| s.borrow_mut().events.push(event));
}
fn events() -> Vec<Event> {
    STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
}
extern "C" fn free_fake(node: *mut u8) -> u32 {
    record(Event::Free(node as usize));
    0xABCD_5678
}
extern "thiscall" fn recurse_fake(this: *mut u8, node: *mut u8) {
    record(Event::Recurse(this as usize, node as usize));
    let (root, replacement) = STATE.with(|s| {
        let s = s.borrow();
        (s.root, s.replacement)
    });
    if root != 0 {
        // SAFETY: this test explicitly provides the writable node whose left link changes during the call.
        unsafe {
            TreeNode::from_ptr(root as *mut u8).set_left(replacement as *mut u8);
        }
    }
}

#[test]
fn each_subtree_helper_calls_its_own_address_and_reloads_left_after_recursion() {
    let _free = fake(0x0090_CF63, free_fake as *const () as usize);
    let mut nodes = [[0usize; 6]; 3];
    let root = nodes[0].as_mut_ptr().cast::<u8>();
    let left = nodes[1].as_mut_ptr().cast::<u8>();
    let right = nodes[2].as_mut_ptr().cast::<u8>();
    let mut map = [0usize; 2];
    let map_ptr = map.as_mut_ptr().cast();
    for (address, function) in [
        (
            0x0069_5CD0,
            fc4p_00695cd0 as unsafe extern "thiscall" fn(*mut u8, *mut u8),
        ),
        (0x0069_5D90, fc4p_00695d90),
        (0x0069_5E10, fc4p_00695e10),
    ] {
        let _recurse = fake(address, recurse_fake as *const () as usize);
        // SAFETY: root, left and right all have complete node layouts. Recursion is faked here.
        unsafe {
            let n = TreeNode::from_ptr(root);
            n.set_left(left);
            n.set_right(right);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.root = root as usize;
                s.replacement = 0;
            });
            function(map_ptr, root);
            assert_eq!(
                events(),
                [
                    Event::Recurse(map_ptr as usize, right as usize),
                    Event::Free(root as usize)
                ]
            );
            function(map_ptr, core::ptr::null_mut());
            assert!(events().is_empty());
        }
    }
}

#[test]
fn subtree_free_order_is_right_node_left_for_all_three_maps() {
    let _free = fake(0x0090_CF63, free_fake as *const () as usize);
    let mut nodes = [[0usize; 6]; 5];
    let base = nodes.as_mut_ptr();
    let p = |i| base.wrapping_add(i) as *mut u8;
    // SAFETY: the five-node tree contains no cycles and each pointer refers to a complete node.
    unsafe {
        TreeNode::from_ptr(p(0)).set_left(p(1));
        TreeNode::from_ptr(p(0)).set_right(p(2));
        TreeNode::from_ptr(p(2)).set_left(p(3));
        TreeNode::from_ptr(p(2)).set_right(p(4));
        for f in [
            fc4p_00695cd0 as unsafe extern "thiscall" fn(*mut u8, *mut u8),
            fc4p_00695d90,
            fc4p_00695e10,
        ] {
            f(core::ptr::null_mut(), p(0));
            assert_eq!(events(), [4, 2, 3, 0, 1].map(|i| Event::Free(p(i) as usize)));
        }
    }
}

extern "C" fn predicate(item: *mut u8) -> u32 {
    record(Event::Predicate(item as usize));
    let hit = STATE.with(|s| s.borrow().hit);
    0xAABB_CC00 | u32::from(item as usize == hit)
}
#[test]
fn candidate_search_covers_groups_tails_and_low_predicate_byte() {
    let mut items = [0u8; 12 * 11];
    let begin = items.as_mut_ptr();
    // SAFETY: each tested range is a prefix of eleven complete candidates.
    unsafe {
        for length in 0..=11 {
            for hit in 0..=length {
                let found = if hit == length { 0 } else { begin.add(hit * 12) as usize };
                STATE.with(|s| s.borrow_mut().hit = found);
                let result = fc4p_00695e50(begin, begin.add(length * 12), predicate);
                assert_eq!(result, begin.add(hit * 12));
                let tested = if hit == length { length } else { hit + 1 };
                let expected: Vec<_> = (0..tested)
                    .map(|i| Event::Predicate(begin.add(i * 12) as usize))
                    .collect();
                assert_eq!(events(), expected);
            }
        }
        assert_eq!(fc4p_00695e50(begin.add(12), begin, predicate), begin);
        assert!(events().is_empty());
    }
}

fn make_tree(nodes: &mut [[u32; 6]], header: *mut u8, keys: &[(i32, i32)], indices: &[usize]) -> *mut u8 {
    if indices.is_empty() {
        return core::ptr::null_mut();
    }
    let middle = indices.len() / 2;
    let index = indices[middle];
    let p = nodes[index].as_mut_ptr().cast::<u8>();
    let left = make_tree(nodes, p, keys, &indices[..middle]);
    let right = make_tree(nodes, p, keys, &indices[middle + 1..]);
    // SAFETY: each node is six words and recursive partitions are disjoint.
    unsafe {
        let n = TreeNode::from_ptr(p);
        n.set_parent(header);
        n.set_left(left);
        n.set_right(right);
        n.set_first(keys[index].0);
        n.set_second(keys[index].1);
    }
    p
}

#[test]
fn profile_and_tract_pair_searches_signed_keys_and_unsigned_stage() {
    let mut nodes = [[0u32; 6]; 7];
    let mut header = [0u32; 6];
    let h = header.as_mut_ptr().cast::<u8>();
    let mut map = [h as usize];
    let m = map.as_mut_ptr().cast::<u8>();
    let keys = [
        (-1, 254),
        (0, 0),
        (0, 128),
        (0, 255),
        (1, 0),
        (i32::MAX, 1),
        (i32::MAX, 255),
    ];
    let root = make_tree(&mut nodes, h, &keys, &[0, 1, 2, 3, 4, 5, 6]);
    // SAFETY: all headers, nodes, and query keys have the original layouts.
    unsafe {
        TreeNode::from_ptr(h).set_parent(root);
        let before = nodes;
        for a in [i32::MIN, -1, 0, 1, 2, i32::MAX] {
            for b in [0, 1, 127, 128, 254, 255] {
                let mut key = [a, b];
                let k = key.as_mut_ptr().cast::<u8>();
                let expected = keys
                    .iter()
                    .position(|&(x, y)| (x, y) == (a, b))
                    .map(|i| nodes[i].as_mut_ptr().cast::<u8>())
                    .unwrap_or(h);
                assert_eq!(fc4p_00695f10(m, k), expected);
                let mut ecx = 0;
                let result = find_tract_pair(m, 0x1234_5678, &mut ecx, k);
                assert_eq!(result as u32, expected as u32);
            }
        }
        assert_eq!(nodes, before);
        let signed_keys = [
            (i32::MIN, i32::MIN),
            (-1, i32::MAX),
            (0, i32::MIN),
            (0, -1),
            (0, 0),
            (0, i32::MAX),
            (i32::MAX, 0),
        ];
        let root = make_tree(&mut nodes, h, &signed_keys, &[0, 1, 2, 3, 4, 5, 6]);
        TreeNode::from_ptr(h).set_parent(root);
        for a in [i32::MIN, -1, 0, 1, i32::MAX] {
            for b in [i32::MIN, -1, 0, 1, i32::MAX] {
                let mut key = [a, b];
                let mut ecx = 0;
                let expected = signed_keys
                    .iter()
                    .position(|&(x, y)| (x, y) == (a, b))
                    .map(|i| nodes[i].as_mut_ptr().cast::<u8>())
                    .unwrap_or(h);
                assert_eq!(
                    find_tract_pair(m, 0, &mut ecx, key.as_mut_ptr().cast()) as u32,
                    expected as u32
                );
            }
        }
        TreeNode::from_ptr(h).set_parent(core::ptr::null_mut());
        let mut key = [1, -1];
        let mut ecx = 123;
        assert_eq!(
            find_tract_pair(m, 0xAABB_CCDD, &mut ecx, key.as_mut_ptr().cast()),
            0xAABB_CCDD_0000_0000 | u64::from(h as u32)
        );
        assert_eq!(ecx, 0);
        assert_eq!(fc4p_00695f10(m, key.as_mut_ptr().cast()), h);
    }
}

#[test]
fn tract_pair_export_restores_ecx_and_edx_results() {
    let mut header = [0u32; 6];
    let h = header.as_mut_ptr().cast::<u8>();
    let mut node = [0u32; 6];
    let n = node.as_mut_ptr().cast::<u8>();
    let mut map = [h as usize];
    let mut key = [7i32, -9];
    // SAFETY: the node and header form a single-entry map. The entry takes one stack word.
    unsafe {
        TreeNode::from_ptr(h).set_parent(n);
        TreeNode::from_ptr(n).set_first(7);
        TreeNode::from_ptr(n).set_second(-9);
        let (eax, edx, ecx): (u32, u32, u32);
        core::arch::asm!("push edx", "call {entry}", entry = sym fc4p_00695f70,
            inlateout("edx") key.as_mut_ptr() => edx, inlateout("ecx") map.as_mut_ptr() => ecx,
            lateout("eax") eax, clobber_abi("C"));
        assert_eq!((eax, edx, ecx), (n as u32, 7, (-9i32) as u32));
    }
}

extern "thiscall" fn find_fake(this: *mut u8, out: *mut *mut u8, key: *mut u32) -> u64 {
    // SAFETY: lookup exports pass their original argument word and a writable iterator.
    unsafe {
        record(Event::Find(this as usize, read_unaligned(key)));
        let found = STATE.with(|s| s.borrow().found);
        write_unaligned(out, found as *mut u8);
    }
    0x1234_5678_AAAA_BBBB
}

#[test]
fn developer_and_last_build_day_delegate_to_matching_map_helpers() {
    let _developer = fake(0x0049_7520, find_fake as *const () as usize);
    let _day = fake(0x006C_0030, find_fake as *const () as usize);
    let mut object = Object::new(0xE4);
    let p = object.ptr();
    let mut header = [0usize; 6];
    let h = header.as_mut_ptr().cast::<u8>();
    let mut node = [0u32; 6];
    node[5] = 0x8765_4321;
    let n = node.as_mut_ptr().cast::<u8>();
    // SAFETY: the simulator headers and the fake lookup output use the game's original offsets.
    unsafe {
        let sim = BuildingSim::from_ptr(p);
        sim.set_developers(h);
        sim.set_last_build_days(h);
        for (entry, offset) in [
            (fc4p_00696c80 as *const () as usize, 0x5C),
            (fc4p_00696cc0 as *const () as usize, 0xB0),
        ] {
            for (found, expected) in [(h, 0), (n, 0x8765_4321)] {
                STATE.with(|s| s.borrow_mut().found = found as usize);
                assert_eq!(call_thiscall_with_eax(entry, p, &[0x3320], 0), (expected, 0x1234_5678));
                assert_eq!(events(), [Event::Find(p.add(offset) as usize, 0x3320)]);
            }
        }
    }
}

#[test]
fn catalogue_hash_collisions_full_eax_and_untouched_miss_output() {
    let mut nodes = [[0u32; 5]; 3];
    let p0 = nodes[0].as_mut_ptr().cast::<u8>();
    let p1 = nodes[1].as_mut_ptr().cast::<u8>();
    let p2 = nodes[2].as_mut_ptr().cast::<u8>();
    nodes[0] = [p1 as u32, 7, 0x1111, 0x2222, 0x3333];
    nodes[1] = [0, 10, 0x4444, 0x5555, 0x6666];
    nodes[2] = [0, u32::MAX, 0xAAAA, 0xBBBB, 0xCCCC];
    std::hint::black_box(&nodes);
    let mut buckets = [p2, p0, core::ptr::null_mut()];
    let mut begin = buckets.as_mut_ptr();
    let mut end = begin.wrapping_add(3);
    let _begin = fake_data(0x00B4_854C, (&mut begin as *mut *mut *mut u8).cast());
    let _end = fake_data(0x00B4_8550, (&mut end as *mut *mut *mut u8).cast());
    // SAFETY: the catalogue has three buckets and the nodes above have full key payloads.
    unsafe {
        for (key, node, value) in [
            (7, p0, [0x1111, 0x2222, 0x3333]),
            (10, p1, [0x4444, 0x5555, 0x6666]),
            (u32::MAX, p2, [0xAAAA, 0xBBBB, 0xCCCC]),
        ] {
            let mut out = [0; 3];
            let out_ptr = out.as_mut_ptr();
            assert_eq!(
                fc4p_00697dd0(core::ptr::null_mut(), key, out_ptr),
                (u64::from(value[2]) << 32) | u64::from((out_ptr as u32 & !255) | 1)
            );
            assert_eq!(out, value);
            assert_eq!(
                fc4p_00697e30(core::ptr::null_mut(), key),
                (u64::from(node as u32) << 32) | 1
            );
        }
        for key in [0, 1, 2, 13, 0x1234_5678] {
            let mut out = [0xDEAD_BEEF; 3];
            assert_eq!(
                fc4p_00697dd0(core::ptr::null_mut(), key, out.as_mut_ptr()),
                u64::from((key / 3) & !255)
            );
            assert_eq!(out, [0xDEAD_BEEF; 3]);
            assert_eq!(fc4p_00697e30(core::ptr::null_mut(), key), 0);
        }
    }
}
