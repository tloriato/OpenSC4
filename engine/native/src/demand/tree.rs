// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Destruction of header-linked map subtrees.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn destroy_subtree(map: *mut u8, node: *mut u8) -> u64 = "thiscall" 0x006A_25C0;
}
game_fn! {
    fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54;
}
game_fn! {
    fn clone_subtree(map: *mut u8, node: *mut u8, parent: *mut u8) -> *mut u8 = "thiscall" 0x006A_2BC0;
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

port! {
    /// Copy the right branches recursively and the left spine iteratively. Copy only the color byte and payload;
    /// allocation padding is retained. The entry restores ECX to the map pointer after the Rust body returns.
    #[port(address = 0x006A_2BC0, owner = "demand", name = "CloneDemandMapSubtree", naked)]
    pub unsafe extern "C" fn fc4p_006a2bc0() {
        core::arch::naked_asm!(
            "push ecx", "push dword ptr [esp + 12]", "push dword ptr [esp + 12]",
            "call {body}", "pop ecx", "ret 8",
            body = sym clone,
        )
    }
}

unsafe fn allocate_copy(source: TreeNode) -> TreeNode {
    unsafe {
        let allocation = allocate(24);
        if !allocation.wrapping_add(16).is_null() {
            write_unaligned(allocation.add(16).cast::<u32>(), source.key());
            write_unaligned(allocation.add(20).cast::<u32>(), source.value());
        }
        let copy = TreeNode::from_ptr(allocation);
        copy.set_color(source.color());
        copy.set_left(core::ptr::null_mut());
        copy.set_right(core::ptr::null_mut());
        copy
    }
}

unsafe extern "thiscall" fn clone(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 {
    unsafe {
        let mut source = TreeNode::from_ptr(source);
        let top = allocate_copy(source);
        top.set_parent(parent);
        let right = source.right();
        if !right.is_null() {
            top.set_right(clone_subtree(this, right, top.as_ptr()));
        }
        let mut next = source.left();
        let mut parent = top;
        while !next.is_null() {
            source = TreeNode::from_ptr(next);
            let copy = allocate_copy(source);
            parent.set_left(copy.as_ptr());
            copy.set_parent(parent.as_ptr());
            let right = source.right();
            if !right.is_null() {
                copy.set_right(clone_subtree(this, right, copy.as_ptr()));
            }
            next = source.left();
            parent = copy;
        }
        top.as_ptr()
    }
}

port! {
    /// Construct a map copy with a new sentinel and copied nodes. Retain unspecified sentinel bytes.
    /// Return the destination in EAX, source count in EDX, and the final boundary node in ECX.
    #[port(address = 0x006A_2DB0, owner = "demand", name = "CopyDemandMap", naked)]
    pub unsafe extern "C" fn fc4p_006a2db0() {
        core::arch::naked_asm!(
            "push edx", "push eax", "mov edx, esp", "call {body}", "mov ecx, dword ptr [esp]", "add esp, 8", "ret 4",
            body = sym copy_map,
        )
    }
}

unsafe extern "fastcall" fn copy_map(this: *mut u8, entry: *mut Entry) -> u64 {
    unsafe {
        let entry = &mut *entry;
        write_unaligned(this.cast::<u32>(), 0);
        let allocation = allocate(24);
        write_unaligned(this.cast::<*mut u8>(), allocation);
        write_unaligned(this.add(4).cast::<u32>(), 0);
        let source = entry.arg(0) as *mut u8;
        let source_header = read_unaligned(source.cast::<*mut u8>());
        let header = TreeNode::from_ptr(allocation);
        header.set_color(0);
        if TreeNode::from_ptr(source_header).parent().is_null() {
            header.set_parent(core::ptr::null_mut());
            header.set_left(allocation);
            header.set_right(allocation);
            entry.eax = source_header as u32;
        } else {
            // The allocator may have changed the source map before the root is read again.
            let source_header = read_unaligned(source.cast::<*mut u8>());
            let root = clone_subtree(this, TreeNode::from_ptr(source_header).parent(), allocation);
            header.set_parent(root);
            let mut first = TreeNode::from_ptr(root);
            while !first.left().is_null() {
                first = TreeNode::from_ptr(first.left());
            }
            header.set_left(first.as_ptr());
            let mut last = TreeNode::from_ptr(header.parent());
            while !last.right().is_null() {
                last = TreeNode::from_ptr(last.right());
            }
            header.set_right(last.as_ptr());
            entry.eax = last.as_ptr() as u32;
        }
        let count = read_unaligned(source.add(4).cast::<u32>());
        write_unaligned(this.add(4).cast(), count);
        registers(this as u32, count)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_006a25c0::PORT, fc4p_006a2bc0::PORT, fc4p_006a2db0::PORT];

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

    #[derive(Debug, PartialEq)]
    enum Call {
        Allocate(u32),
        Clone(u32, u32, u32),
    }
    #[derive(Default)]
    struct State {
        // Allocations stay at fixed addresses as the collection grows.
        #[allow(clippy::vec_box)]
        allocations: Vec<Box<[u8; 24]>>,
        calls: Vec<Call>,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    extern "C" fn allocation(size: u32) -> *mut u8 {
        assert_eq!(size, 24);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.calls.push(Call::Allocate(size));
            let mut allocation = Box::new([0xCC; 24]);
            let pointer = allocation.as_mut_ptr();
            s.allocations.push(allocation);
            pointer
        })
    }
    extern "thiscall" fn clone_call(map: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 {
        STATE.with(|s| {
            s.borrow_mut()
                .calls
                .push(Call::Clone(map as u32, source as u32, parent as u32))
        });
        let result = unsafe {
            crate::testing::call_thiscall_with_eax(
                fc4p_006a2bc0 as *const () as usize,
                map,
                &[source as u32, parent as u32],
                0,
            )
        };
        result.0 as *mut u8
    }

    unsafe fn assert_copy(source: *mut u8, copy: *mut u8, parent: *mut u8) {
        unsafe {
            if source.is_null() {
                assert!(copy.is_null());
                return;
            }
            assert_ne!(source, copy);
            let a = TreeNode::from_ptr(source);
            let b = TreeNode::from_ptr(copy);
            assert_eq!(b.parent(), parent);
            assert_eq!((a.color(), a.key(), a.value()), (b.color(), b.key(), b.value()));
            assert_eq!(std::slice::from_raw_parts(copy.add(1), 3), &[0xCC; 3]);
            assert_copy(a.left(), b.left(), copy);
            assert_copy(a.right(), b.right(), copy);
        }
    }

    #[test]
    fn subtree_copy_uses_game_allocations_preserves_padding_and_restores_ecx() {
        use crate::demand::{interface::tests::Map, test_support::Probe};
        let _allocator = fake(0x0090_CF54, allocation as *const () as usize);
        let _clone = fake(0x006A_2BC0, clone_call as *const () as usize);
        STATE.with(|s| *s.borrow_mut() = State::default());
        let mut source = Map::new(&[(1, 10), (2, 20), (3, 30), (4, 40), (5, 50), (6, 60), (7, 70)]);
        for (i, node) in source.memory.iter_mut().enumerate().skip(1) {
            node[0] = 0xAABB_CC00 | (i as u32 & 1);
        }
        let root = unsafe { TreeNode::from_ptr(source.header()).parent() };
        let map = 0x1234_5678usize as *mut u8;
        let parent = 0xABCD_EF00usize as *mut u8;
        let mut call = Probe::new(fc4p_006a2bc0 as *const (), map, [root as u32, parent as u32, 0x55]);
        unsafe {
            call.run();
            assert_copy(root, call.eax as *mut u8, parent);
        }
        assert_eq!(call.ecx, map as u32);
        assert_eq!(call.after, call.args);
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(s.allocations.len(), 7);
            let pointers: Vec<_> = s.allocations.iter().map(|a| a.as_ptr() as u32).collect();
            assert_eq!(s.calls[0], Call::Allocate(24));
            let root_right = unsafe { TreeNode::from_ptr(root).right() };
            assert_eq!(s.calls[1], Call::Clone(map as u32, root_right as u32, pointers[0]));
            assert_eq!(s.calls.iter().filter(|c| matches!(c, Call::Clone(..))).count(), 3);
        });
    }

    #[test]
    fn map_copy_builds_header_extrema_and_returns_count_and_boundary_registers() {
        use crate::demand::{interface::tests::Map, test_support::Probe};
        let _allocator = fake(0x0090_CF54, allocation as *const () as usize);
        let _clone = fake(0x006A_2BC0, clone_call as *const () as usize);
        for empty in [true, false] {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let values = [(1, 10), (2, 20), (3, 30)];
            let mut source = Map::new(if empty { &[] } else { &values });
            let header = source.header();
            let mut source_map = [header as u32, if empty { 0 } else { 3 }, 0xCAFE];
            let mut destination = [0xDEAD_BEEF; 3];
            let mut call = Probe::new(
                fc4p_006a2db0 as *const (),
                destination.as_mut_ptr().cast(),
                [source_map.as_mut_ptr() as u32, 0x77, 0x88],
            );
            unsafe {
                call.run();
            }
            assert_eq!(call.eax, destination.as_ptr() as u32);
            assert_eq!(call.edx, source_map[1]);
            assert_eq!(destination[1..], [source_map[1], 0xDEAD_BEEF]);
            assert_eq!(call.after, call.args);
            let output = unsafe { TreeNode::from_ptr(destination[0] as *mut u8) };
            assert_eq!(output.color(), 0);
            unsafe {
                assert_copy(TreeNode::from_ptr(header).parent(), output.parent(), output.as_ptr());
            }
            if empty {
                assert_eq!(call.ecx, header as u32);
                assert_eq!((output.left(), output.right()), (output.as_ptr(), output.as_ptr()));
            } else {
                assert_eq!(call.ecx, output.right() as u32);
                assert_eq!(unsafe { TreeNode::from_ptr(output.left()).key() }, 1);
                assert_eq!(unsafe { TreeNode::from_ptr(output.right()).key() }, 3);
            }
            let bytes = unsafe { std::slice::from_raw_parts(output.as_ptr(), 24) };
            assert_eq!(&bytes[1..4], &[0xCC; 3]);
            assert_eq!(&bytes[16..24], &[0xCC; 8]);
        }
    }
}
