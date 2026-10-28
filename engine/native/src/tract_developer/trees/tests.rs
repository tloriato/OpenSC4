// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::{fake, Object};
use crate::tract_developer::testing::{run, snapshot, EAX, ECX, EDX};
use std::cell::RefCell;

#[test]
fn rotations_update_root_parent_child_links_and_keep_other_bytes() {
    for (address, left) in [(0x0040D540, true), (0x0040D590, false)] {
        for position in 0..3 {
            for middle_present in [false, true] {
                let mut objects: Vec<_> = (0..5).map(|_| Object::new(0x20)).collect();
                let ptrs: Vec<_> = objects.iter_mut().map(|obj| obj.ptr() as u32).collect();
                let (n, promoted, middle, parent, sibling) = (ptrs[0], ptrs[1], ptrs[2], ptrs[3], ptrs[4]);
                let mut root = if position == 0 { n } else { parent };
                let root_ptr = &mut root as *mut u32 as u32;
                let child_offset = if left { 0xC } else { 8 };
                let middle_offset = if left { 8 } else { 0xC };
                unsafe {
                    for p in &ptrs {
                        for i in 0..0x20 {
                            *(*p as *mut u8).add(i) = 0xA5;
                        }
                    }
                    store(n, 4, parent);
                    store(n, child_offset, promoted);
                    store(promoted, middle_offset, if middle_present { middle } else { 0 });
                    store(parent, 8, if position == 1 { n } else { sibling });
                    store(parent, 0xC, if position == 2 { n } else { sibling });
                    store(middle, 4, promoted);
                }
                let mut want: Vec<_> = ptrs.iter().map(|&p| snapshot(p as *mut u8, 0x20)).collect();
                let set = |bytes: &mut Vec<u8>, offset: u32, value: u32| {
                    bytes[offset as usize..offset as usize + 4].copy_from_slice(&value.to_le_bytes());
                };
                set(&mut want[0], child_offset, if middle_present { middle } else { 0 });
                set(&mut want[0], 4, promoted);
                set(&mut want[1], middle_offset, n);
                set(&mut want[1], 4, parent);
                if middle_present {
                    set(&mut want[2], 4, n);
                }
                if position != 0 {
                    set(&mut want[3], if position == 1 { 8 } else { 0xC }, promoted);
                }
                assert_eq!(
                    run(address, Conv::Cdecl, ECX, &[n, root_ptr]),
                    (promoted, n, if position == 0 { root_ptr } else { parent })
                );
                assert_eq!(root, if position == 0 { promoted } else { parent });
                for (&p, expected) in ptrs.iter().zip(want) {
                    assert_eq!(snapshot(p as *mut u8, 0x20), expected);
                }
            }
        }
    }
}

thread_local! {
    static CALLS: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
    static RECURSIVE: RefCell<Vec<(u32, u32)>> = const { RefCell::new(Vec::new()) };
}

extern "C" fn free(p: *mut u8) -> u64 {
    // SAFETY: fake free sees nodes that remain allocated by the test.
    let id = unsafe { word(p as u32, 0x10) };
    CALLS.with(|calls| calls.borrow_mut().push((p as u32, id)));
    0x1122_3344_5566_7788
}

#[test]
fn subtree_disposal_frees_right_before_node_before_left_without_other_writes() {
    let _free = fake(0x0090CF63, free as *const () as usize);
    for address in [0x00704990, 0x007049D0, 0x00704A10] {
        assert_eq!(run(address, Conv::Thiscall, ECX, &[0]), (EAX, ECX, EDX));
        let mut objects: Vec<_> = (0..7).map(|_| Object::new(0x20)).collect();
        let p: Vec<_> = objects.iter_mut().map(|obj| obj.ptr() as u32).collect();
        for (i, &node) in p.iter().enumerate() {
            unsafe {
                store(node, 0x10, i as u32);
            }
        }
        unsafe {
            store(p[0], 8, p[1]);
            store(p[0], 0xC, p[2]);
            store(p[1], 8, p[3]);
            store(p[1], 0xC, p[4]);
            store(p[2], 8, p[5]);
            store(p[2], 0xC, p[6]);
        }
        let before: Vec<_> = p.iter().map(|&node| snapshot(node as *mut u8, 0x20)).collect();
        let result = run(address, Conv::Thiscall, ECX, &[p[0]]);
        assert_eq!(result.0, 0x5566_7788);
        assert_eq!(result.2, 0x1122_3344);
        CALLS.with(|calls| {
            assert_eq!(
                std::mem::take(&mut *calls.borrow_mut()),
                [6, 2, 5, 0, 4, 1, 3].map(|i| (p[i], i as u32))
            )
        });
        for (&node, expected) in p.iter().zip(before) {
            assert_eq!(snapshot(node as *mut u8, 0x20), expected);
        }
    }
}

extern "thiscall" fn recurse(this: u32, child: u32) -> u64 {
    RECURSIVE.with(|calls| calls.borrow_mut().push((this, child)));
    0x8765_4321_AABB_CCDD
}

#[test]
fn subtree_disposal_calls_original_recursive_entry_with_tree_and_right_child() {
    let _free = fake(0x0090CF63, free as *const () as usize);
    for address in [0x00704990, 0x007049D0, 0x00704A10] {
        let export = crate::PORTS.iter().find(|port| port.address == address).unwrap().entry as usize;
        let _recursive = fake(address, recurse as *const () as usize);
        let mut object = Object::new(0x20);
        let node = object.ptr() as u32;
        unsafe {
            store(node, 0xC, 0xBAD0_CAFE);
        }
        let mut r = Entry {
            eax: EAX,
            ecx: ECX,
            edx: EDX,
            return_address: 0,
            args: [],
        };
        unsafe {
            invoke(&mut r, Conv::Thiscall, export, &[node]);
        }
        assert_eq!(r.eax, 0x5566_7788);
        RECURSIVE.with(|calls| assert_eq!(std::mem::take(&mut *calls.borrow_mut()), [(ECX, 0xBAD0_CAFE)]));
        CALLS.with(|calls| assert_eq!(std::mem::take(&mut *calls.borrow_mut()), [(node, 0)]));
    }
}
