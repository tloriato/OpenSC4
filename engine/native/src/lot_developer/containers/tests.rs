// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::testing::fake;
use std::cell::RefCell;

struct Script {
    object: *mut u8,
    parent: *mut u8,
    key: i32,
    result: *mut u8,
    predecessor_from: *mut u8,
    predecessor_to: *mut u8,
    calls: Vec<&'static str>,
}
thread_local! { static SCRIPT: RefCell<Option<Script>> = const { RefCell::new(None) }; }

unsafe extern "thiscall" fn insert(
    this: *mut u8,
    output: *mut *mut u8,
    child: *mut u8,
    parent: *mut u8,
    key: *const i32,
    left: u32,
) -> u64 {
    SCRIPT.with(|state| {
        let mut state = state.borrow_mut();
        let script = state.as_mut().unwrap();
        assert_eq!(this, script.object);
        assert!(child.is_null());
        assert_eq!(parent, script.parent);
        assert_eq!(unsafe { key.read_unaligned() }, script.key);
        assert_eq!(left, 0);
        unsafe { output.write_unaligned(script.result) };
        script.calls.push("insert");
    });
    (0xAABB_CCDDu64 << 32) | u64::from(output as u32)
}

unsafe extern "thiscall" fn previous(iterator: *mut *mut u8) -> u64 {
    SCRIPT.with(|state| {
        let mut state = state.borrow_mut();
        let script = state.as_mut().unwrap();
        assert_eq!(unsafe { iterator.read_unaligned() }, script.predecessor_from);
        unsafe { iterator.write_unaligned(script.predecessor_to) };
        script.calls.push("previous");
    });
    0x1234_5678_9ABC_DEF0
}

#[test]
fn signed_tree_search_inserts_or_returns_existing_and_retains_result_padding() {
    let _insert = fake(0x0072_BC80, insert as *const () as usize);
    let _previous = fake(0x0068_09E0, previous as *const () as usize);
    unsafe {
        let mut header = [0u32; 5];
        let mut root = [0u32; 5];
        let mut left = [0u32; 5];
        let mut right = [0u32; 5];
        let hp = header.as_mut_ptr().cast::<u8>();
        let rp = root.as_mut_ptr().cast::<u8>();
        let lp = left.as_mut_ptr().cast::<u8>();
        let xp = right.as_mut_ptr().cast::<u8>();
        let mut object = [hp, core::ptr::null_mut()];
        let this = object.as_mut_ptr().cast::<u8>();
        let inserted = 0x1234_5678usize as *mut u8;
        RuleNode::from_ptr(hp).set_parent(rp);
        RuleNode::from_ptr(hp).set_left(lp);
        RuleNode::from_ptr(rp).set_left(lp);
        RuleNode::from_ptr(rp).set_right(xp);
        RuleNode::from_ptr(rp).set_key(10);
        RuleNode::from_ptr(lp).set_key(5);
        RuleNode::from_ptr(xp).set_key(20);
        for (key, parent, predecessor, existing, leftmost) in [
            (i32::MIN, lp, core::ptr::null_mut(), core::ptr::null_mut(), true),
            (4, lp, core::ptr::null_mut(), core::ptr::null_mut(), true),
            (5, lp, core::ptr::null_mut(), lp, false),
            (7, lp, core::ptr::null_mut(), core::ptr::null_mut(), false),
            (10, xp, rp, rp, false),
            (15, xp, rp, core::ptr::null_mut(), false),
            (20, xp, core::ptr::null_mut(), xp, false),
            (i32::MAX, xp, core::ptr::null_mut(), core::ptr::null_mut(), false),
        ] {
            SCRIPT.with(|s| {
                *s.borrow_mut() = Some(Script {
                    object: this,
                    parent,
                    key,
                    result: inserted,
                    predecessor_from: xp,
                    predecessor_to: predecessor,
                    calls: vec![],
                })
            });
            let mut output = [0xA5u8; 8];
            let returned = fc4p_006c6ae0(this, output.as_mut_ptr(), &key);
            let result = if existing.is_null() { inserted } else { existing };
            assert_eq!(output.as_ptr().cast::<*mut u8>().read_unaligned(), result);
            assert_eq!(&output[4..], &[u8::from(existing.is_null()), 0xA5, 0xA5, 0xA5]);
            assert_eq!(returned as u32, output.as_mut_ptr() as u32);
            assert_eq!(
                returned >> 32,
                if leftmost { 0xAABB_CCDD } else { result as u32 as u64 }
            );
            let calls = SCRIPT.with(|s| s.borrow_mut().take().unwrap().calls);
            let mut expected = Vec::new();
            if !predecessor.is_null() {
                expected.push("previous");
            }
            if existing.is_null() {
                expected.push("insert");
            }
            assert_eq!(calls, expected);
        }
        // The empty header is its own leftmost node.
        RuleNode::from_ptr(hp).set_parent(core::ptr::null_mut());
        RuleNode::from_ptr(hp).set_left(hp);
        SCRIPT.with(|s| {
            *s.borrow_mut() = Some(Script {
                object: this,
                parent: hp,
                key: -1,
                result: inserted,
                predecessor_from: core::ptr::null_mut(),
                predecessor_to: core::ptr::null_mut(),
                calls: vec![],
            })
        });
        let mut output = [0xA5u8; 8];
        assert_eq!(fc4p_006c6ae0(this, output.as_mut_ptr(), &-1) >> 32, 0xAABB_CCDD);
        assert_eq!(SCRIPT.with(|s| s.borrow_mut().take().unwrap().calls), ["insert"]);
        assert_eq!(output[4], 1);
    }
}
