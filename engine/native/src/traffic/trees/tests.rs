// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::traffic::test_support::{bytes, invoke};
use std::cell::RefCell;

#[derive(Debug, PartialEq, Eq)]
enum Call { Allocate(u32), Balance(usize, usize), Insert(usize, usize, usize, u16, u32), Previous(usize), Clone(usize, usize, usize), Erase(usize, usize) }
#[derive(Default)]
struct State { calls: Vec<Call>, allocations: Vec<usize>, next: usize, previous: usize, inserted: usize, clone: usize }
thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
fn reset() { STATE.with(|s| *s.borrow_mut() = State::default()); }
fn calls() -> Vec<Call> { STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls)) }
unsafe extern "C" fn allocation(size: u32) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.calls.push(Call::Allocate(size));
        let p = s.allocations[s.next]; s.next += 1; p as *mut u8
    })
}
unsafe extern "C" fn balance(node: *mut u8, root: *mut u32) -> u64 {
    unsafe {
        assert_eq!(word(node, 8), 0); assert_eq!(word(node, 12), 0);
        assert!(!root.read_unaligned().eq(&0));
    }
    STATE.with(|s| s.borrow_mut().calls.push(Call::Balance(node as usize, root as usize)));
    0x1122_3344_5566_7788
}
unsafe extern "thiscall" fn insertion(this: *mut u8, out: *mut u32, hint: *mut u8, parent: *mut u8, key: *mut u16, flag: u32) -> u64 {
    STATE.with(|s| {
        let mut s = s.borrow_mut();
        s.calls.push(Call::Insert(this as usize, hint as usize, parent as usize, unsafe { key.read_unaligned() }, flag));
        unsafe { out.write_unaligned(s.inserted as u32); }
    });
    (0xAABB_CCDDu64 << 32) | out as u64
}
unsafe extern "thiscall" fn previous(iterator: *mut u32) {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.calls.push(Call::Previous(unsafe { iterator.read_unaligned() } as usize));
        unsafe { iterator.write_unaligned(s.previous as u32); }
    });
}
#[unsafe(naked)]
unsafe extern "C" fn previous_with_iterator_ecx() {
    core::arch::naked_asm!("push ecx", "call {body}", "pop ecx", "ret", body = sym previous);
}
unsafe extern "thiscall" fn cloning(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 {
    STATE.with(|s| s.borrow_mut().calls.push(Call::Clone(this as usize, source as usize, parent as usize)));
    unsafe { invoke(fc4p_00710b80 as *const () as usize, this, &[source as u32, parent as u32], 0, 0, false).eax as *mut u8 }
}
unsafe extern "thiscall" fn clone_stub(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 {
    STATE.with(|s| {
        let mut s = s.borrow_mut(); s.calls.push(Call::Clone(this as usize, source as usize, parent as usize));
        s.clone as *mut u8
    })
}
unsafe extern "thiscall" fn erasing(this: *mut u8, root: *mut u8) {
    unsafe { assert_eq!(word(this, 4), 9); }
    STATE.with(|s| s.borrow_mut().calls.push(Call::Erase(this as usize, root as usize)));
}
struct Tree { object: Object, header: Object }
impl Tree {
    fn new() -> Self {
        let mut tree = Self { object: Object::new(8), header: Object::new(24) };
        unsafe {
            store(tree.object.ptr(), 0, tree.header.ptr() as u32);
            store(tree.header.ptr(), 8, tree.header.ptr() as u32);
            store(tree.header.ptr(), 12, tree.header.ptr() as u32);
        }
        tree
    }
}
unsafe fn node(p: *mut u8, key: u16, parent: *mut u8, left: *mut u8, right: *mut u8) {
    unsafe { store(p, 4, parent as u32); store(p, 8, left as u32); store(p, 12, right as u32); write_unaligned(p.add(16).cast::<u16>(), key); }
}

#[test]
fn word_node_insertion_handles_root_minimum_maximum_and_force_right() {
    let _allocate = fake(0x0090_CF54, allocation as *const () as usize);
    let _balance = fake(0x0040_D5E0, balance as *const () as usize);
    for scenario in 0..5 {
        reset();
        let mut t = Tree::new();
        let mut parent = Object::new(24);
        let mut allocated = Object::new(20);
        let mut key = 10u16;
        let mut destination = 0u32;
        unsafe {
            core::ptr::write_bytes(allocated.ptr(), 0xA5, 20);
            node(parent.ptr(), if scenario == 3 { 5 } else { 20 }, t.header.ptr(), core::ptr::null_mut(), core::ptr::null_mut());
            if scenario != 0 {
                store(t.header.ptr(), 4, parent.ptr() as u32);
                store(t.header.ptr(), 8, parent.ptr() as u32);
                store(t.header.ptr(), 12, parent.ptr() as u32);
            }
            let parent = if scenario == 0 { t.header.ptr() } else { parent.ptr() };
            let hint = if scenario == 2 { 1 } else { 0 };
            let force = u32::from(scenario == 4);
            STATE.with(|s| s.borrow_mut().allocations.push(allocated.ptr() as usize));
            let result = invoke(fc4p_0070fe00 as *const () as usize, t.object.ptr(), &[
                &mut destination as *mut u32 as u32, hint, parent as u32, &mut key as *mut u16 as u32, force,
            ], 0xABCD, 0x1234, false);
            assert_eq!((result.eax, result.edx), (&mut destination as *mut u32 as u32, 0x1122_3344));
            assert_eq!(destination, allocated.ptr() as u32);
            let left = scenario < 3;
            assert_eq!(word(parent, if left { 8 } else { 12 }), allocated.ptr() as u32);
            assert_eq!(word(t.header.ptr(), if left { 8 } else { 12 }), allocated.ptr() as u32);
            assert_eq!(word(t.object.ptr(), 4), 1);
            assert_eq!(word(allocated.ptr(), 4), parent as u32);
            assert_eq!(read_unaligned(allocated.ptr().add(16).cast::<u16>()), key);
            let written = bytes(allocated.ptr(), 20);
            assert_eq!(&written[0..4], &[0xA5; 4]); assert_eq!(&written[18..20], &[0xA5; 2]);
            assert_eq!(calls(), [Call::Allocate(20), Call::Balance(allocated.ptr() as usize, t.header.ptr().add(4) as usize)]);
        }
    }
}

#[test]
fn unique_word_insertion_handles_empty_duplicate_minimum_predecessor_and_successor() {
    let _insert = fake(0x0070_FE00, insertion as *const () as usize);
    let _previous = fake(0x0068_09E0, previous as *const () as usize);
    for scenario in 0..5 {
        reset();
        let mut t = Tree::new();
        let mut root = Object::new(24);
        let mut predecessor = Object::new(24);
        let mut inserted = Object::new(20);
        let mut result = [0xA5u8; 8];
        let mut key = match scenario { 1 => 20, 2 => 10, 3 => 25, _ => 30 };
        unsafe {
            node(root.ptr(), 20, t.header.ptr(), core::ptr::null_mut(), core::ptr::null_mut());
            node(predecessor.ptr(), 22, t.header.ptr(), core::ptr::null_mut(), core::ptr::null_mut());
            if scenario != 0 {
                store(t.header.ptr(), 4, root.ptr() as u32);
                store(t.header.ptr(), 8, if scenario == 3 { predecessor.ptr() as u32 } else { root.ptr() as u32 });
            }
            // Scenario 3 descends left from a larger root, then checks its predecessor.
            if scenario == 3 { write_unaligned(root.ptr().add(16).cast::<u16>(), 28); }
            STATE.with(|s| { let mut s = s.borrow_mut(); s.previous = predecessor.ptr() as usize; s.inserted = inserted.ptr() as usize; });
            let regs = invoke(fc4p_007109b0 as *const () as usize, t.object.ptr(), &[result.as_mut_ptr() as u32, &mut key as *mut u16 as u32], 0, 0, false);
            assert_eq!(regs.eax, result.as_ptr() as u32);
            let insertion = scenario != 1;
            assert_eq!(result[4], u8::from(insertion)); assert_eq!(&result[5..], &[0xA5; 3]);
            assert_eq!(word(result.as_mut_ptr(), 0), if insertion { inserted.ptr() as u32 } else { root.ptr() as u32 });
            let expected_parent = if scenario == 0 { t.header.ptr() } else { root.ptr() };
            let mut expected = Vec::new();
            if scenario == 3 { expected.push(Call::Previous(root.ptr() as usize)); }
            if insertion { expected.push(Call::Insert(t.object.ptr() as usize, 0, expected_parent as usize, key, 0)); }
            assert_eq!(calls(), expected);
            if scenario == 0 || scenario == 2 { assert_eq!(regs.ecx, inserted.ptr() as u32); }
            if scenario == 3 || scenario == 4 { assert_eq!(regs.edx, inserted.ptr() as u32); }
            if scenario == 1 { assert_eq!((regs.ecx, regs.edx), (t.header.ptr() as u32, root.ptr() as u32)); }
        }
    }
}

#[test]
fn clone_pair_tree_allocates_right_subtrees_before_left_spines_and_preserves_padding() {
    reset();
    let _allocate = fake(0x0090_CF54, allocation as *const () as usize);
    let _clone = fake(0x0071_0B80, cloning as *const () as usize);
    let mut t = Tree::new();
    let mut sources: Vec<_> = (0..5).map(|_| Object::new(24)).collect();
    let mut outputs: Vec<_> = (0..5).map(|_| Object::new(24)).collect();
    unsafe {
        for (i, output) in outputs.iter_mut().enumerate() {
            core::ptr::write_bytes(output.ptr(), 0xA5, 24);
            sources[i].ptr().write(i as u8);
            store(sources[i].ptr(), 16, i as u32 + 100);
            store(sources[i].ptr(), 20, i as u32 + 200);
        }
        store(sources[0].ptr(), 12, sources[1].ptr() as u32);
        store(sources[1].ptr(), 8, sources[2].ptr() as u32);
        store(sources[0].ptr(), 8, sources[3].ptr() as u32);
        store(sources[3].ptr(), 12, sources[4].ptr() as u32);
        STATE.with(|s| s.borrow_mut().allocations = outputs.iter_mut().map(|o| o.ptr() as usize).collect());
        let regs = invoke(fc4p_00710b80 as *const () as usize, t.object.ptr(), &[sources[0].ptr() as u32, t.header.ptr() as u32], 0, 0, false);
        assert_eq!((regs.eax, regs.ecx), (outputs[0].ptr() as u32, t.object.ptr() as u32));
        assert_eq!(calls(), [
            Call::Allocate(24), Call::Clone(t.object.ptr() as usize, sources[1].ptr() as usize, outputs[0].ptr() as usize),
            Call::Allocate(24), Call::Allocate(24), Call::Allocate(24),
            Call::Clone(t.object.ptr() as usize, sources[4].ptr() as usize, outputs[3].ptr() as usize), Call::Allocate(24),
        ]);
        for (i, output) in outputs.iter_mut().enumerate() {
            assert_eq!(output.ptr().read(), i as u8);
            assert_eq!(&bytes(output.ptr(), 24)[1..4], &[0xA5; 3]);
            assert_eq!((word(output.ptr(), 16), word(output.ptr(), 20)), (i as u32 + 100, i as u32 + 200));
        }
        assert_eq!(word(outputs[0].ptr(), 4), t.header.ptr() as u32);
        for (parent, offset, child) in [(0, 12, 1), (1, 8, 2), (0, 8, 3), (3, 12, 4)] {
            assert_eq!(word(outputs[parent].ptr(), offset), outputs[child].ptr() as u32);
            assert_eq!(word(outputs[child].ptr(), 4), outputs[parent].ptr() as u32);
        }
    }
}

#[test]
fn assignment_handles_self_empty_and_nonempty_trees_and_rebuilds_extremes() {
    let _clone = fake(0x0071_0B80, clone_stub as *const () as usize);
    let _erase = fake(0x0042_6A60, erasing as *const () as usize);
    for empty_source in [true, false] {
        reset();
        let mut destination = Tree::new(); let mut source = Tree::new();
        let mut root = Object::new(24); let mut left = Object::new(24); let mut right = Object::new(24);
        unsafe {
            store(destination.object.ptr(), 4, 9); store(destination.header.ptr(), 4, 0x1234);
            let before = bytes(destination.object.ptr(), 8);
            assert_eq!(fc4p_00710ff0(destination.object.ptr(), destination.object.ptr()), destination.object.ptr());
            assert_eq!(bytes(destination.object.ptr(), 8), before); assert!(calls().is_empty());
            if !empty_source {
                store(source.object.ptr(), 4, 3); store(source.header.ptr(), 4, 0x5678);
                node(root.ptr(), 2, destination.header.ptr(), left.ptr(), right.ptr());
                STATE.with(|s| s.borrow_mut().clone = root.ptr() as usize);
            }
            assert_eq!(fc4p_00710ff0(destination.object.ptr(), source.object.ptr()), destination.object.ptr());
            let mut expected = vec![Call::Erase(destination.object.ptr() as usize, 0x1234)];
            if empty_source {
                assert_eq!(word(destination.object.ptr(), 4), 0);
                assert_eq!(word(destination.header.ptr(), 4), 0);
                assert_eq!((word(destination.header.ptr(), 8), word(destination.header.ptr(), 12)), (destination.header.ptr() as u32, destination.header.ptr() as u32));
            } else {
                expected.push(Call::Clone(destination.object.ptr() as usize, 0x5678, destination.header.ptr() as usize));
                assert_eq!(word(destination.object.ptr(), 4), 3);
                assert_eq!((word(destination.header.ptr(), 4), word(destination.header.ptr(), 8), word(destination.header.ptr(), 12)), (root.ptr() as u32, left.ptr() as u32, right.ptr() as u32));
            }
            assert_eq!(calls(), expected);
        }
    }
}

#[test]
fn pointer_range_walks_empty_partial_and_segmented_ranges() {
    unsafe {
        assert_eq!(fc4p_0070fec0(0x1000, 0, 0x1080, core::ptr::null_mut(), 0x1000), 0x1000);
        assert_eq!(fc4p_0070fec0(0x1000, 0, 0x1080, core::ptr::null_mut(), 0x107C), 0x107C);
        let mut blocks = [0x1000u32, 0x2000, 0x3000];
        assert_eq!(fc4p_0070fec0(0x107C, 0, 0x1080, blocks.as_mut_ptr(), 0x300C), 0x300C);
    }
}

#[test]
fn unique_word_search_uses_the_original_argument_slot_for_its_iterator() {
    reset();
    let _previous = fake(0x0068_09E0, previous_with_iterator_ecx as *const () as usize);
    let mut tree = Tree::new(); let mut root = Object::new(24); let mut predecessor = Object::new(24);
    let mut key = 21u16; let mut destination = [0u32; 2];
    let mut record = [&mut key as *mut u16 as u32, destination.as_mut_ptr() as u32, 0, 0];
    unsafe {
        node(root.ptr(), 28, tree.header.ptr(), core::ptr::null_mut(), core::ptr::null_mut());
        node(predecessor.ptr(), 22, tree.header.ptr(), core::ptr::null_mut(), core::ptr::null_mut());
        store(tree.header.ptr(), 4, root.ptr() as u32); store(tree.header.ptr(), 8, predecessor.ptr() as u32);
        STATE.with(|s| s.borrow_mut().previous = predecessor.ptr() as usize);
        core::arch::asm!(
            "push dword ptr [edi]", "push dword ptr [edi + 4]", "lea eax, [esp + 4]", "mov [edi + 8], eax",
            "call {target}", "mov [edi + 12], ecx", target = sym fc4p_007109b0,
            in("edi") record.as_mut_ptr(), inlateout("ecx") tree.object.ptr() => _, clobber_abi("C"),
        );
        assert_eq!(destination, [predecessor.ptr() as u32, 0]);
    }
    assert_eq!(record[2], record[3]);
    assert_eq!(calls(), [Call::Previous(root.ptr() as usize)]);
}
