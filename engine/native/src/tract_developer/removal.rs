// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Shared red-black tree removal used by the tract suitability cache.

use super::*;

game_fn! { #[allow(dead_code)] fn rotate_left(node: u32, root: u32) -> u32 = "C" 0x0040_D540; }
game_fn! { #[allow(dead_code)] fn rotate_right(node: u32, root: u32) -> u32 = "C" 0x0040_D590; }

unsafe fn color(node: u32) -> u8 {
    // SAFETY: each caller established a nonnull live node.
    unsafe { *(node as *const u8) }
}
unsafe fn set_color(node: u32, value: u8) {
    // SAFETY: each caller established a nonnull live node with a writable color byte.
    unsafe {
        *(node as *mut u8) = value;
    }
}

/// Promote a red sibling as part of removal. This rotation is performed within the removal itself.
unsafe fn red_sibling<const N: usize>(r: &mut Entry<N>, parent: u32, root: u32, left_case: bool) {
    // SAFETY: the sibling is live, and all nonnull links refer to live tree nodes.
    unsafe {
        let near = if left_case { 8 } else { 0xC };
        let far = if left_case { 0xC } else { 8 };
        set_color(r.eax, 1);
        r.eax = word(parent, far);
        set_color(parent, 0);
        r.edx = word(r.eax, near);
        store(parent, far, r.edx);
        r.ecx = word(r.eax, near);
        if r.ecx != 0 {
            store(r.ecx, 4, parent);
        }
        r.ecx = word(parent, 4);
        store(r.eax, 4, r.ecx);
        if parent == word(root, 0) {
            store(root, 0, r.eax);
        } else {
            r.ecx = word(parent, 4);
            if parent == word(r.ecx, near) {
                store(r.ecx, near, r.eax);
            } else {
                store(r.ecx, far, r.eax);
            }
        }
        store(r.eax, near, parent);
        store(parent, 4, r.eax);
        r.eax = word(parent, far);
    }
}

register_port! {
    /// Unlink one node without freeing it. Move its successor into place when both children exist,
    /// swap only the color bytes, maintain the root and extrema, and repair black-height deficits.
    /// Return the original node for the caller to dispose.
    0x0040_DAF0, fc4p_0040daf0, unlink_node, 4, 0, "UnlinkAndRebalanceTreeNode", |r| {
        let [node, root, first, last] = r.args;
        r.eax = node; r.edx = word(r.eax, 8);
        let mut removed = r.eax;
        let replacement;
        let mut parent;
        if r.edx == 0 {
            replacement = word(r.eax, 0xC);
        } else {
            r.ecx = word(r.eax, 0xC);
            if r.ecx == 0 { replacement = r.edx; }
            else {
                removed = r.ecx; r.ecx = word(removed, 8);
                while r.ecx != 0 { removed = r.ecx; r.ecx = word(removed, 8); }
                replacement = word(removed, 0xC);
            }
        }
        if removed != node {
            store(r.edx, 4, removed); r.ecx = word(node, 8); store(removed, 8, r.ecx);
            if removed != word(node, 0xC) {
                parent = word(removed, 4);
                if replacement != 0 { store(replacement, 4, parent); }
                r.edx = word(removed, 4); store(r.edx, 8, replacement);
                r.ecx = word(node, 0xC); store(removed, 0xC, r.ecx);
                r.edx = word(node, 0xC); store(r.edx, 4, removed);
            } else { parent = removed; }
            r.ecx = root;
            if word(r.ecx, 0) == node { store(r.ecx, 0, removed); }
            else {
                r.ecx = word(node, 4);
                if word(r.ecx, 8) == node { store(r.ecx, 8, removed); }
                else { store(r.ecx, 0xC, removed); }
            }
            r.ecx = word(node, 4); store(removed, 4, r.ecx);
            r.ecx = (r.ecx & !0xFF) | u32::from(color(removed));
            r.edx = (r.edx & !0xFF) | u32::from(color(node));
            set_color(removed, r.edx as u8); set_color(node, r.ecx as u8);
            removed = node;
        } else {
            parent = word(removed, 4);
            if replacement != 0 { store(replacement, 4, parent); }
            r.ecx = root;
            if word(r.ecx, 0) == node { store(r.ecx, 0, replacement); }
            else {
                r.ecx = word(node, 4);
                if word(r.ecx, 8) == node { store(r.ecx, 8, replacement); }
                else { store(r.ecx, 0xC, replacement); }
            }
            if word(first, 0) == node {
                r.ecx = word(node, 0xC);
                if r.ecx == 0 { r.ecx = word(node, 4); store(first, 0, r.ecx); }
                else {
                    r.ecx = word(replacement, 8); r.edx = replacement;
                    while r.ecx != 0 { r.edx = r.ecx; r.ecx = word(r.edx, 8); }
                    store(first, 0, r.edx);
                }
            }
            r.edx = last;
            if word(r.edx, 0) == node {
                r.ecx = word(node, 8);
                if r.ecx == 0 { r.eax = word(node, 4); store(r.edx, 0, r.eax); }
                else {
                    r.eax = word(replacement, 0xC); r.ecx = replacement;
                    while r.eax != 0 { r.ecx = r.eax; r.eax = word(r.ecx, 0xC); }
                    store(r.edx, 0, r.ecx);
                }
            }
        }
        if color(removed) == 0 { r.eax = removed; return; }
        let mut child = replacement;
        r.ecx = root;
        if child != word(r.ecx, 0) {
            loop {
                if child != 0 && color(child) != 1 { break; }
                r.eax = word(parent, 8);
                let left_case = child == r.eax;
                let near = if left_case { 8 } else { 0xC };
                let far = if left_case { 0xC } else { 8 };
                if left_case { r.eax = word(parent, 0xC); }
                if color(r.eax) == 0 { red_sibling(r, parent, root, left_case); }
                r.ecx = word(r.eax, near);
                let near_black = r.ecx == 0 || color(r.ecx) == 1;
                let both_black = if near_black {
                    r.ecx = word(r.eax, far); r.ecx == 0 || color(r.ecx) == 1
                } else { false };
                if both_black {
                    set_color(r.eax, 0); r.eax = word(root, 0);
                    child = parent; parent = word(parent, 4);
                    if child == r.eax { break; }
                    continue;
                }
                r.ecx = word(r.eax, far);
                if r.ecx == 0 || color(r.ecx) == 1 {
                    r.ecx = word(r.eax, near);
                    if r.ecx != 0 { set_color(r.ecx, 1); }
                    set_color(r.eax, 0);
                    let rotation = if left_case { 0x0040_D590 } else { 0x0040_D540 };
                    invoke(r, Conv::Cdecl, crate::abi::target(rotation), &[r.eax, root]);
                    r.eax = word(parent, far);
                }
                if left_case {
                    r.edx = (r.edx & !0xFF) | u32::from(color(parent)); set_color(r.eax, r.edx as u8);
                } else {
                    r.ecx = (r.ecx & !0xFF) | u32::from(color(parent)); set_color(r.eax, r.ecx as u8);
                }
                set_color(parent, 1); r.eax = word(r.eax, far);
                if r.eax != 0 { set_color(r.eax, 1); }
                let rotation = if left_case { 0x0040_D540 } else { 0x0040_D590 };
                invoke(r, Conv::Cdecl, crate::abi::target(rotation), &[parent, root]);
                break;
            }
        }
        r.eax = removed;
        if child != 0 { set_color(child, 1); }
    }
}

pub const PORTS: &[Port] = &[fc4p_0040daf0::PORT];

#[cfg(test)]
mod tests;
