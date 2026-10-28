// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Destruction of header-linked map subtrees.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn destroy_subtree(map: *mut u8, node: *mut u8) -> u64 = "thiscall" 0x006A_25C0;
}
game_fn! {
    #[allow(dead_code)]
    fn free(pointer: *mut u8) -> u64 = "C" 0x0090_CF63;
}

port! {
    /// Destroy each right subtree, free its root, then continue down the left spine. Null roots retain EAX and EDX.
    #[port(address = 0x006A_25C0, owner = "demand", name = "DestroyDemandMapSubtree", naked)]
    pub unsafe extern "C" fn fc4p_006a25c0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 4",
            body = sym destroy,
        )
    }
}

unsafe extern "fastcall" fn destroy(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &*entry;
        let mut node = entry.arg(0) as *mut u8;
        let (mut eax, mut edx) = (entry.eax, entry.edx);
        while !node.is_null() {
            let right = TreeNode::from_ptr(node).right() as u32;
            let args = [right];
            let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006A_25C0), this as u32, &args);
            call.eax = right;
            call.edx = edx;
            call_raw(&mut call);
            let left = TreeNode::from_ptr(node).left();
            let args = [node as u32];
            let mut release = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
            release.eax = call.eax;
            release.edx = call.edx;
            call_raw(&mut release);
            eax = release.eax;
            edx = release.edx;
            node = left;
        }
        registers(eax, edx)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a25c0::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fake;
    use std::cell::RefCell;
    thread_local! {
        static FREED: RefCell<Vec<u32>> = const { RefCell::new(Vec::new()) };
    }
    extern "C" fn release(node: *mut u8) -> u64 {
        FREED.with(|f| f.borrow_mut().push(node as u32));
        registers(node as u32 ^ 0xABCD_1234, node as u32 ^ 0x7654_3210)
    }
    #[test]
    fn subtree_release_follows_right_root_left_order_without_freeing_the_header() {
        let _fake = fake(0x0090_CF63, release as *const () as usize);
        let mut map = crate::demand::interface::tests::Map::new(&[(1, 10), (2, 20), (3, 30)]);
        let header = map.header();
        let root = unsafe { TreeNode::from_ptr(header).parent() };
        let left = unsafe { TreeNode::from_ptr(root).left() };
        let right = unsafe { TreeNode::from_ptr(root).right() };
        let args = [root as u32];
        let mut call = RawCall::new(Conv::Thiscall, fc4p_006a25c0 as *const () as usize, 0x1234, &args);
        call.edx = 0xAABB_CCDD;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!(FREED.with(|f| f.take()), [right as u32, root as u32, left as u32]);
        assert_eq!(
            (call.eax, call.edx),
            (left as u32 ^ 0xABCD_1234, left as u32 ^ 0x7654_3210)
        );
        let args = [0];
        let mut call = RawCall::new(Conv::Thiscall, fc4p_006a25c0 as *const () as usize, 0x1234, &args);
        call.eax = 0x1122_3344;
        call.edx = 0xAABB_CCDD;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!((call.eax, call.edx), (0x1122_3344, 0xAABB_CCDD));
        assert!(FREED.with(|f| f.take()).is_empty());
    }
}
