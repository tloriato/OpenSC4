// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Shared tree rotations and three subtree disposal helpers.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn free_node(node: *mut u8) -> u32 = "C" 0x0090_CF63;
}
game_fn! {
    #[allow(dead_code)]
    fn dispose_lots(tree: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x0070_4990;
}
game_fn! {
    #[allow(dead_code)]
    fn dispose_configurations(tree: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x0070_49D0;
}
game_fn! {
    #[allow(dead_code)]
    fn dispose_cache(tree: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x0070_4A10;
}

register_port! {
    /// Promote the right child in a left rotation, retaining the replacement root and parent registers.
    0x0040_D540, fc4p_0040d540, rotate_left, 2, 0, "TreeRotateLeft", |r| {
        r.ecx = r.args[0];
        let node = Node::from_ptr(r.ecx as *mut u8);
        r.eax = node.right();
        let promoted = Node::from_ptr(r.eax as *mut u8);
        r.edx = promoted.left();
        node.set_right(r.edx);
        r.edx = promoted.left();
        if r.edx != 0 { Node::from_ptr(r.edx as *mut u8).set_parent(r.ecx); }
        r.edx = node.parent();
        promoted.set_parent(r.edx);
        r.edx = r.args[1];
        if r.ecx == word(r.edx, 0) {
            store(r.edx, 0, r.eax);
        } else {
            r.edx = node.parent();
            let parent = Node::from_ptr(r.edx as *mut u8);
            if r.ecx == parent.left() { parent.set_left(r.eax); }
            else { parent.set_right(r.eax); }
        }
        promoted.set_left(r.ecx);
        node.set_parent(r.eax);
    }
}

register_port! {
    /// Promote the left child in a right rotation, retaining the replacement root and parent registers.
    0x0040_D590, fc4p_0040d590, rotate_right, 2, 0, "TreeRotateRight", |r| {
        r.ecx = r.args[0];
        let node = Node::from_ptr(r.ecx as *mut u8);
        r.eax = node.left();
        let promoted = Node::from_ptr(r.eax as *mut u8);
        r.edx = promoted.right();
        node.set_left(r.edx);
        r.edx = promoted.right();
        if r.edx != 0 { Node::from_ptr(r.edx as *mut u8).set_parent(r.ecx); }
        r.edx = node.parent();
        promoted.set_parent(r.edx);
        r.edx = r.args[1];
        if r.ecx == word(r.edx, 0) {
            store(r.edx, 0, r.eax);
        } else {
            r.edx = node.parent();
            let parent = Node::from_ptr(r.edx as *mut u8);
            if r.ecx == parent.right() { parent.set_right(r.eax); }
            else { parent.set_left(r.eax); }
        }
        promoted.set_right(r.ecx);
        node.set_parent(r.eax);
    }
}

unsafe fn dispose<const N: usize>(r: &mut Entry<N>, address: u32) {
    // SAFETY: each entry supplies a tree object and either a null pointer or a live subtree root.
    unsafe {
        let this = r.ecx;
        let mut node = r.args[0];
        while node != 0 {
            r.eax = word(node, 0xC);
            r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(address), &[r.eax]);
            let next = word(node, 8);
            invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF63), &[node]);
            node = next;
        }
    }
}

register_port! {
    /// Dispose right subtrees recursively, then each node while following the left spine.
    0x0070_4990, fc4p_00704990, dispose_lot_tree, 1, 4, "DisposeLotTree", |r| {
        dispose(r, 0x0070_4990);
    }
}
register_port! {
    /// Dispose configuration nodes without releasing their stored pointers.
    0x0070_49D0, fc4p_007049d0, dispose_configuration_tree, 1, 4, "DisposeConfigurationTree", |r| {
        dispose(r, 0x0070_49D0);
    }
}
register_port! {
    /// Dispose suitability-cache nodes without changing their tree header.
    0x0070_4A10, fc4p_00704a10, dispose_cache_tree, 1, 4, "DisposeSuitabilityTree", |r| {
        dispose(r, 0x0070_4A10);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0040d540::PORT,
    fc4p_0040d590::PORT,
    fc4p_00704990::PORT,
    fc4p_007049d0::PORT,
    fc4p_00704a10::PORT,
];

#[cfg(test)]
mod tests;
