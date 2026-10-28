// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Shared tree insertion, cloning and assignment used by traffic containers.

use super::*;
use super::network::{register_port, registers};
use crate::abi::{Conv, RawCall};

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn rebalance(node: *mut u8, root: *mut u32) = "C" 0x0040_D5E0; }
game_fn! { #[allow(dead_code)] fn insert(this: *mut u8, out: *mut u32, hint: *mut u8, parent: *mut u8, key: *mut u16, force_right: u32) -> *mut u32 = "thiscall" 0x0070_FE00; }
game_fn! { #[allow(dead_code)] fn predecessor(iterator: *mut u32) = "thiscall" 0x0068_09E0; }
game_fn! { fn clone_tree(this: *mut u8, source: *mut u8, parent: *mut u8) -> *mut u8 = "thiscall" 0x0071_0B80; }
game_fn! { fn erase_tree(this: *mut u8, root: *mut u8) = "thiscall" 0x0042_6A60; }

register_port!(0x0070_FE00, fc4p_0070fe00, "InsertTrafficWordTreeNode", insert_node, 20, [24, 20, 16, 12, 8]);
unsafe extern "fastcall" fn insert_node(this: *mut u8, out: *mut Registers, destination: *mut u32, hint: *mut u8, parent: *mut u8, key: *mut u16, force_right: u32) {
    unsafe {
        let left = parent == word(this, 0) as *mut u8 || (force_right == 0 &&
            (!hint.is_null() || read_unaligned(key) < read_unaligned(parent.add(0x10).cast::<u16>())));
        let allocation = registers(RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF54), 0, &[20]));
        let node = allocation.eax as *mut u8;
        let value = node.wrapping_add(0x10);
        if !value.is_null() { write_unaligned(value.cast::<u16>(), read_unaligned(key)); }
        if left {
            store(parent, 8, node as u32);
            let header = word(this, 0) as *mut u8;
            if parent == header {
                store(header, 4, node as u32);
                store(word(this, 0) as *mut u8, 12, node as u32);
            } else if parent == word(header, 8) as *mut u8 {
                store(header, 8, node as u32);
            }
        } else {
            store(parent, 12, node as u32);
            let header = word(this, 0) as *mut u8;
            if parent == word(header, 12) as *mut u8 { store(header, 12, node as u32); }
        }
        store(node, 4, parent as u32);
        store(node, 8, 0);
        store(node, 12, 0);
        let root = (word(this, 0) as *mut u8).add(4);
        let args = [node as u32, root as u32];
        let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0040_D5E0), word(this, 0), &args);
        call.eax = node as u32;
        call.edx = root as u32;
        let mut result = registers(call);
        store(this, 4, word(this, 4).wrapping_add(1));
        write_unaligned(destination, node as u32);
        result.eax = destination as u32;
        out.write(result);
    }
}

port! {
    /// Walks a segmented pointer range without destroying its pointed-to elements.
    #[port(address = 0x0070_FEC0, owner = "traffic", name = "WalkTrafficPointerRange")]
    pub unsafe extern "C" fn fc4p_0070fec0(current: u32, _first: u32, end: u32, block: *mut u32, last: u32) -> u32 {
        let (mut current, mut end, mut block) = (current, end, block);
        while current != last {
            current = current.wrapping_add(4);
            if current == end {
                unsafe { current = read_unaligned(block.wrapping_add(1)); }
                block = block.wrapping_add(1);
                end = current.wrapping_add(128);
            }
        }
        current
    }
}

register_port!(0x0071_09B0, fc4p_007109b0, "InsertUniqueTrafficWord", insert_unique, 8, [12, 8]);
unsafe extern "fastcall" fn insert_unique(this: *mut u8, out: *mut Registers, destination: *mut u8, key: *mut u16) {
    unsafe {
        let header = word(this, 0) as *mut u8;
        let mut node = word(header, 4) as *mut u8;
        let mut parent = header;
        let mut left = true;
        while !node.is_null() {
            left = read_unaligned(key) < read_unaligned(node.add(0x10).cast::<u16>());
            parent = node;
            node = word(node, if left { 8 } else { 12 }) as *mut u8;
        }
        let mut candidate = parent as u32;
        let mut result = Registers { eax: 0, ecx: header as u32, edx: candidate };
        let at_minimum = left && parent == word(header, 8) as *mut u8;
        if left && !at_minimum {
            let args = [&mut candidate as *mut u32 as u32];
            result = registers(RawCall::new(Conv::Thiscall, crate::abi::target(0x0068_09E0), args[0], &[]));
            result.edx = candidate;
        }
        if at_minimum || read_unaligned((candidate as *mut u8).add(0x10).cast::<u16>()) < read_unaligned(key) {
            let mut inserted = candidate;
            let args = [&mut inserted as *mut u32 as u32, node as u32, parent as u32, key as u32, 0];
            result = registers(RawCall::new(Conv::Thiscall, crate::abi::target(0x0070_FE00), this as u32, &args));
            // The insertion helper returns the address of its output, which need not be the input address.
            let inserted = word(result.eax as *mut u8, 0);
            if at_minimum { result.ecx = inserted; } else { result.edx = inserted; }
            store(destination, 0, inserted);
            destination.add(4).write(1);
        } else {
            store(destination, 0, candidate);
            destination.add(4).write(0);
        }
        result.eax = destination as u32;
        out.write(result);
    }
}

register_port!(0x0071_0B80, fc4p_00710b80, "CloneTrafficPairTree", clone_nodes, 8, [12, 8]);
unsafe extern "fastcall" fn clone_nodes(this: *mut u8, out: *mut Registers, source: *mut u8, parent: *mut u8) {
    unsafe {
        let root = allocate(24);
        let value = root.wrapping_add(16);
        if !value.is_null() {
            store(value, 0, word(source, 16));
            store(value, 4, word(source, 20));
        }
        root.write(source.read());
        store(root, 8, 0);
        store(root, 12, 0);
        store(root, 4, parent as u32);
        let right = word(source, 12) as *mut u8;
        if !right.is_null() { store(root, 12, clone_tree(this, right, root) as u32); }
        let mut source = word(source, 8) as *mut u8;
        let mut parent = root;
        while !source.is_null() {
            let node = allocate(24);
            let value = node.wrapping_add(16);
            if !value.is_null() {
                store(value, 0, word(source, 16));
                store(value, 4, word(source, 20));
            }
            node.write(source.read());
            store(node, 8, 0);
            store(node, 12, 0);
            store(parent, 8, node as u32);
            store(node, 4, parent as u32);
            let right = word(source, 12) as *mut u8;
            if !right.is_null() { store(node, 12, clone_tree(this, right, node) as u32); }
            source = word(source, 8) as *mut u8;
            parent = node;
        }
        out.write(Registers { eax: root as u32, ecx: this as u32, edx: 0 });
    }
}

port! {
    /// Clears the destination tree, clones the source and rebuilds its extreme links. Self assignment
    /// leaves every byte and allocation alone.
    #[port(address = 0x0071_0FF0, owner = "traffic", name = "AssignTrafficPairTree")]
    pub unsafe extern "thiscall" fn fc4p_00710ff0(this: *mut u8, source: *mut u8) -> *mut u8 {
        unsafe {
            if this == source { return this; }
            if word(this, 4) != 0 {
                let header = word(this, 0) as *mut u8;
                erase_tree(this, word(header, 4) as *mut u8);
                let header = word(this, 0) as *mut u8;
                store(header, 8, header as u32);
                store(word(this, 0) as *mut u8, 4, 0);
                let header = word(this, 0) as *mut u8;
                store(header, 12, header as u32);
                store(this, 4, 0);
            }
            store(this, 4, 0);
            let root = word(word(source, 0) as *mut u8, 4) as *mut u8;
            if root.is_null() {
                store(word(this, 0) as *mut u8, 4, 0);
                let header = word(this, 0) as *mut u8;
                store(header, 8, header as u32);
                let header = word(this, 0) as *mut u8;
                store(header, 12, header as u32);
            } else {
                let header = word(this, 0) as *mut u8;
                store(header, 4, clone_tree(this, root, header) as u32);
                let header = word(this, 0) as *mut u8;
                let mut node = word(header, 4) as *mut u8;
                while word(node, 8) != 0 { node = word(node, 8) as *mut u8; }
                store(header, 8, node as u32);
                let header = word(this, 0) as *mut u8;
                let mut node = word(header, 4) as *mut u8;
                while word(node, 12) != 0 { node = word(node, 12) as *mut u8; }
                store(header, 12, node as u32);
                store(this, 4, word(source, 4));
            }
            this
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070fe00::PORT, fc4p_0070fec0::PORT, fc4p_007109b0::PORT, fc4p_00710b80::PORT, fc4p_00710ff0::PORT];

#[cfg(test)]
mod tests;
