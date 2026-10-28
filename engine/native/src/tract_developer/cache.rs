// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Ordered pairs used to cache configuration filters.

use super::*;

game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn rebalance(node: u32, root: u32) -> u32 = "C" 0x0040_D5E0; }
game_fn! { #[allow(dead_code)] fn predecessor(iterator: u32) -> u32 = "thiscall" 0x0068_09E0; }
game_fn! { #[allow(dead_code)] fn successor(iterator: u32) -> u32 = "thiscall" 0x005D_17C0; }
game_fn! { #[allow(dead_code)] fn compare_pairs(this: u32, a: u32, b: u32) -> u32 = "thiscall" 0x0063_37B0; }
game_fn! { #[allow(dead_code)] fn lower_bound(this: u32, pair: u32) -> u32 = "thiscall" 0x0063_C070; }
game_fn! { #[allow(dead_code)] fn insert_node(this: u32, output: u32, x: u32, parent: u32, value: u32, force: u32) -> u32 = "thiscall" 0x0070_5B80; }
game_fn! { #[allow(dead_code)] fn insert_unique(this: u32, output: u32, value: u32) -> u32 = "thiscall" 0x0070_5C60; }
game_fn! { #[allow(dead_code)] fn insert_hint(this: u32, output: u32, hint: u32, value: u32) -> u32 = "thiscall" 0x0070_6040; }

register_port! {
    /// Allocate a node, copy its two signed key words and payload, link it on the chosen side,
    /// rebalance through the shared game helper, then increment the tree's count.
    0x0070_5B80, fc4p_00705b80, attach_node, 5, 20, "InsertConfigurationCacheNode", |r| {
        let this = r.ecx;
        let [output, x, parent, value, force] = r.args;
        r.eax = word(this, 0);
        let left = if parent == r.eax { true } else {
            r.eax = force;
            if r.eax != 0 { false } else {
                r.eax = x;
                if r.eax != 0 { true } else {
                    r.eax = word(value, 0); r.ecx = word(parent, 0x10);
                    if (r.ecx as i32) > r.eax as i32 { true }
                    else if (r.ecx as i32) < r.eax as i32 { false }
                    else { r.eax = word(value, 4); (r.eax as i32) < word(parent, 0x14) as i32 }
                }
            }
        };
        invoke(r, Conv::Cdecl, crate::abi::target(0x0090_CF54), &[0x1C]);
        r.ecx = r.eax.wrapping_add(0x10);
        if r.ecx != 0 {
            for offset in [0, 4, 8] { r.edx = word(value, offset); store(r.ecx, offset, r.edx); }
        }
        let node = r.eax;
        store(parent, if left { 8 } else { 0xC }, node);
        r.ecx = word(this, 0);
        if left {
            if parent == r.ecx {
                store(r.ecx, 4, r.eax);
                r.ecx = word(this, 0); store(r.ecx, 0xC, r.eax);
            } else if parent == word(r.ecx, 8) { store(r.ecx, 8, r.eax); }
        } else if parent == word(r.ecx, 0xC) { store(r.ecx, 0xC, r.eax); }
        store(node, 4, parent); store(node, 8, 0); store(node, 0xC, 0);
        r.edx = word(this, 0).wrapping_add(4);
        invoke(r, Conv::Cdecl, crate::abi::target(0x0040_D5E0), &[node, r.edx]);
        r.eax = word(this, 4).wrapping_add(1); store(this, 4, r.eax);
        r.eax = output; store(r.eax, 0, node);
    }
}

register_port! {
    /// Find a pair's insertion position with signed lexicographic comparisons. Return the existing
    /// node for duplicates; otherwise attach one node and set only the result's low success byte.
    0x0070_5C60, fc4p_00705c60, unique_pair, 2, 8, "InsertUniqueConfigurationCachePair", |r| {
        let this = r.ecx;
        let [output, value] = r.args;
        r.edx = word(this, 0);
        let mut x = word(r.edx, 4);
        let mut parent = r.edx;
        r.eax = (r.eax & !0xFF) | 1;
        if x != 0 {
            r.ecx = word(value, 0);
            loop {
                r.eax = word(x, 0x10); parent = x;
                let left = if (r.eax as i32) > r.ecx as i32 { true }
                    else if (r.eax as i32) < r.ecx as i32 { false }
                    else { r.eax = word(value, 4); (r.eax as i32) < word(x, 0x14) as i32 };
                x = word(x, if left { 8 } else { 0xC });
                r.eax = (r.eax & !0xFF) | u32::from(left);
                if x == 0 { break; }
            }
        }
        r.ecx = parent;
        let mut iterator = r.ecx;
        let mut insert = false;
        if r.eax as u8 != 0 {
            r.ecx = &mut iterator as *mut u32 as u32;
            if parent == word(r.edx, 8) { insert = true; }
            else {
                invoke(r, Conv::Thiscall, crate::abi::target(0x0068_09E0), &[]);
                r.ecx = iterator;
            }
        }
        if !insert {
            r.eax = word(r.ecx, 0x10); r.edx = word(value, 0);
            insert = if (r.edx as i32) > r.eax as i32 { true }
                else if (r.edx as i32) < r.eax as i32 { false }
                else { r.eax = word(r.ecx, 0x14); (r.eax as i32) < word(value, 4) as i32 };
            if insert { r.ecx = &mut iterator as *mut u32 as u32; }
        }
        if insert {
            let destination = r.ecx; r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_5B80), &[destination, x, parent, value, 0]);
            r.edx = word(r.eax, 0); r.eax = output;
            store(r.eax, 0, r.edx); *(r.eax.wrapping_add(4) as *mut u8) = 1;
        } else {
            r.eax = output; store(r.eax, 0, r.ecx); *(r.eax.wrapping_add(4) as *mut u8) = 0;
        }
    }
}

unsafe fn attach_hint<const N: usize>(
    r: &mut Entry<N>,
    this: u32,
    output: u32,
    x: u32,
    parent: u32,
    value: u32,
    force: u32,
) {
    // SAFETY: the enclosing port established the parent and insertion side.
    unsafe {
        r.ecx = this;
        invoke(
            r,
            Conv::Thiscall,
            crate::abi::target(0x0070_5B80),
            &[output, x, parent, value, force],
        );
        r.eax = output;
    }
}

unsafe fn unique_hint<const N: usize>(r: &mut Entry<N>, this: u32, output: u32, value: u32, use_ecx: bool) {
    // SAFETY: the original unique-insert helper supplies a node/success result in this local buffer.
    unsafe {
        let mut result = [0u32; 2];
        r.ecx = &mut result as *mut [u32; 2] as u32;
        let destination = r.ecx;
        r.ecx = this;
        invoke(
            r,
            Conv::Thiscall,
            crate::abi::target(0x0070_5C60),
            &[destination, value],
        );
        if use_ecx {
            r.ecx = word(r.eax, 0);
            r.eax = output;
            store(output, 0, r.ecx);
        } else {
            r.edx = word(r.eax, 0);
            r.eax = output;
            store(output, 0, r.edx);
        }
    }
}

register_port! {
    /// Use a position hint when the key fits between its neighbors. Duplicate keys return their
    /// existing node; unsuitable hints fall back to the full unique-insertion helper.
    0x0070_6040, fc4p_00706040, hinted_pair, 3, 12, "InsertHintedConfigurationCachePair", |r| {
        let this = r.ecx;
        let [output, hint, value] = r.args;
        r.eax = word(this, 0);
        if hint == word(r.eax, 8) {
            r.eax = word(this, 4);
            if r.eax == 0 { r.eax = value; unique_hint(r, this, output, value, false); return; }
            r.eax = word(value, 0); r.ecx = word(hint, 0x10);
            let less = if (r.ecx as i32) > r.eax as i32 { true }
                else if (r.ecx as i32) < r.eax as i32 { false }
                else { r.ecx = word(value, 4); (r.ecx as i32) < word(hint, 0x14) as i32 };
            if less { attach_hint(r, this, output, hint, hint, value, 0); return; }
            r.ecx = word(hint, 0x10);
            let greater = if (r.eax as i32) > r.ecx as i32 { true }
                else if (r.eax as i32) < r.ecx as i32 { false }
                else { r.edx = word(hint, 0x14); (r.edx as i32) < word(value, 4) as i32 };
            if !greater { r.eax = output; store(output, 0, hint); return; }
            let mut next = hint; r.ecx = &mut next as *mut u32 as u32;
            invoke(r, Conv::Thiscall, crate::abi::target(0x005D_17C0), &[]);
            if next == word(this, 0) { attach_hint(r, this, output, 0, hint, value, hint); return; }
            r.eax = next.wrapping_add(0x10); r.ecx = this.wrapping_add(8);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0063_37B0), &[value, r.eax]);
            if r.eax as u8 == 0 { unique_hint(r, this, output, value, false); return; }
            r.eax = word(hint, 0xC); r.ecx = this;
            if r.eax == 0 { attach_hint(r, this, output, 0, hint, value, hint); }
            else { attach_hint(r, this, output, next, next, value, 0); }
            return;
        }
        if hint == r.eax {
            r.eax = word(r.eax, 0xC); r.ecx = value; r.edx = word(r.eax, 0x10);
            let greater = if (word(value, 0) as i32) > r.edx as i32 { true }
                else if (word(value, 0) as i32) < r.edx as i32 { false }
                else { r.edx = word(r.eax, 0x14); (r.edx as i32) < word(value, 4) as i32 };
            if greater { attach_hint(r, this, output, 0, r.eax, value, hint); }
            else { unique_hint(r, this, output, value, true); }
            return;
        }
        let mut neighbor = hint;
        r.ecx = &mut neighbor as *mut u32 as u32;
        invoke(r, Conv::Thiscall, crate::abi::target(0x0068_09E0), &[]);
        r.eax = word(value, 0); r.ecx = word(hint, 0x10);
        let less = if (r.ecx as i32) > r.eax as i32 { true }
            else if (r.ecx as i32) < r.eax as i32 { false }
            else { r.edx = word(value, 4); (r.edx as i32) < word(hint, 0x14) as i32 };
        let lower_flag;
        if less {
            r.ecx = neighbor; r.edx = word(r.ecx, 0x10); lower_flag = true;
            let after_previous = if (r.eax as i32) > r.edx as i32 { true }
                else if (r.eax as i32) < r.edx as i32 { false }
                else { r.eax = word(r.ecx, 0x14); (r.eax as i32) < word(value, 4) as i32 };
            if after_previous {
                r.eax = word(r.ecx, 0xC);
                if r.eax != 0 { attach_hint(r, this, output, hint, hint, value, 0); }
                else { attach_hint(r, this, output, 0, r.ecx, value, r.ecx); }
                return;
            }
        } else { lower_flag = false; }
        neighbor = hint; r.ecx = &mut neighbor as *mut u32 as u32;
        invoke(r, Conv::Thiscall, crate::abi::target(0x005D_17C0), &[]);
        r.ecx = (r.ecx & !0xFF) | u32::from(lower_flag);
        r.eax = (r.eax & !0xFF) | u32::from(lower_flag);
        if lower_flag { unique_hint(r, this, output, value, false); return; }
        r.eax = hint.wrapping_add(0x10); r.ecx = this.wrapping_add(8);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0063_37B0), &[r.eax, value]);
        if r.eax as u8 == 0 { r.eax = output; store(output, 0, hint); return; }
        r.ecx = word(this, 0); r.eax = neighbor;
        if r.eax != r.ecx {
            r.eax = r.eax.wrapping_add(0x10); r.ecx = this.wrapping_add(8);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0063_37B0), &[value, r.eax]);
            if r.eax as u8 == 0 { unique_hint(r, this, output, value, false); return; }
            r.eax = neighbor;
        }
        r.ecx = word(hint, 0xC); r.ecx = this;
        if word(hint, 0xC) == 0 { attach_hint(r, this, output, 0, hint, value, hint); }
        else { attach_hint(r, this, output, r.eax, r.eax, value, 0); }
    }
}

register_port! {
    /// Return a pair's payload address, inserting a zero payload with the lower-bound hint if absent.
    0x0070_74D0, fc4p_007074d0, pair_value, 1, 4, "ConfigurationCacheValue", |r| {
        let this = r.ecx;
        let key = r.args[0];
        invoke(r, Conv::Thiscall, crate::abi::target(0x0063_C070), &[key]);
        let mut insert = r.eax == word(this, 0);
        if !insert {
            r.ecx = word(key, 0); r.edx = word(r.eax, 0x10);
            insert = if (r.edx as i32) > r.ecx as i32 { true }
                else if (r.edx as i32) < r.ecx as i32 { false }
                else { r.ecx = word(key, 4); (r.ecx as i32) < word(r.eax, 0x14) as i32 };
        }
        if insert {
            r.edx = word(key, 0); r.ecx = word(key, 4);
            let mut value = [r.edx, r.ecx, 0];
            r.edx = value.as_mut_ptr() as u32;
            let hint = r.eax;
            let mut output = 0u32;
            r.eax = &mut output as *mut u32 as u32;
            let result = r.eax; r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0070_6040), &[result, hint, r.edx]);
            r.eax = output;
        }
        r.eax = r.eax.wrapping_add(0x18);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00705b80::PORT,
    fc4p_00705c60::PORT,
    fc4p_00706040::PORT,
    fc4p_007074d0::PORT,
];

#[cfg(test)]
mod tests;
