// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Signed-key insertion into the boundary-rule tree.

use super::*;

layout! {
    /// Header and node links used by the rule tree. The header's root shares the parent offset.
    pub struct RuleNode {
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => key / set_key: i32,
    }
}

game_fn! { fn insert_rule(this: *mut u8, output: *mut *mut u8, child: *mut u8, parent: *mut u8, key: *const i32, force_left: u32) -> u64 = "thiscall" 0x0072_BC80; }
game_fn! { fn predecessor(iterator: *mut *mut u8) -> u64 = "thiscall" 0x0068_09E0; }

port! {
    /// Finds an existing signed key or inserts it at the search's parent. The result is a node pointer
    /// and an inserted byte; its three padding bytes are retained.
    #[port(address = 0x006C_6AE0, owner = "lotdeveloper", name = "InsertBoundaryRule")]
    pub unsafe extern "thiscall" fn fc4p_006c6ae0(this: *mut u8, output: *mut u8, key: *const i32) -> u64 {
        unsafe {
            let header = this.cast::<*mut u8>().read_unaligned();
            let mut child = RuleNode::from_ptr(header).parent();
            let mut parent = header;
            let mut less = true;
            if !child.is_null() {
                let value = key.read_unaligned();
                while !child.is_null() {
                    let node = RuleNode::from_ptr(child);
                    less = value < node.key();
                    parent = child;
                    child = if less { node.left() } else { node.right() };
                }
            }
            let mut candidate = parent;
            if less {
                if parent == RuleNode::from_ptr(header).left() {
                    let returned = insert_rule(this, &mut candidate, child, parent, key, 0);
                    let inserted = (returned as u32 as *mut *mut u8).read_unaligned();
                    output.cast::<*mut u8>().write_unaligned(inserted);
                    output.add(4).write(1);
                    return (returned & 0xFFFF_FFFF_0000_0000) | u64::from(output as u32);
                }
                predecessor(&mut candidate);
            }
            if RuleNode::from_ptr(candidate).key() < key.read_unaligned() {
                let returned = insert_rule(this, &mut candidate, child, parent, key, 0);
                let inserted = (returned as u32 as *mut *mut u8).read_unaligned();
                output.add(4).write(1);
                output.cast::<*mut u8>().write_unaligned(inserted);
                (u64::from(inserted as u32) << 32) | u64::from(output as u32)
            } else {
                output.add(4).write(0);
                output.cast::<*mut u8>().write_unaligned(candidate);
                (u64::from(candidate as u32) << 32) | u64::from(output as u32)
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c6ae0::PORT];

#[cfg(test)]
mod tests;
