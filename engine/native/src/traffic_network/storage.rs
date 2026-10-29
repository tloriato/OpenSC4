// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Reference release and the map's owned node lists.

use super::{table_word, virtual_words, words, TileInfo, TrafficNetworkMap};
use crate::port::Port;

layout! {
    /// Circular list links and the retained object in a list node.
    pub struct AutomatonLink {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => object / set_object: *mut u8,
    }
}

layout! {
    /// Tree node links; the right branch is visited before the left branch.
    struct TreeNode {
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
    }
}

layout! {
    /// The category word used to update a departing automaton's counters.
    struct Automaton {
        0x1E4 => category / set_category: u32,
    }
}

game_fn! { fn free_node(node: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn destroy_tree(this: *mut u8, node: *mut u8) -> u64 = "thiscall" 0x004A_FAD0; }
game_fn! { fn allocate_records(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }

layout! {
    /// Three-pointer vector of 16-byte traffic records.
    pub struct RecordVector {
        0x00 => begin / set_begin: *mut u8,
        0x04 => end / set_end: *mut u8,
        0x08 => capacity / set_capacity: *mut u8,
    }
}

unsafe fn copy_vector(destination: *mut u8, source: *mut u8, end_slot: *mut u32) {
    unsafe {
        let source = RecordVector::from_ptr(source);
        let count = (source.end() as i32).wrapping_sub(source.begin() as i32) >> 4;
        let destination = RecordVector::from_ptr(destination);
        destination.set_begin(core::ptr::null_mut());
        destination.set_end(core::ptr::null_mut());
        destination.set_capacity(core::ptr::null_mut());
        let bytes = (count as u32).wrapping_shl(4);
        let mut output = if count == 0 {
            core::ptr::null_mut()
        } else {
            allocate_records(bytes)
        };
        destination.set_begin(output);
        destination.set_end(output);
        destination.set_capacity(output.wrapping_add(bytes as usize));
        // Re-read the source after allocation and the header writes. Sources and destinations may overlap.
        let mut input = source.begin();
        let end = source.end();
        if !end_slot.is_null() {
            end_slot.write(end as u32);
        }
        while input != end {
            if !output.is_null() {
                for offset in [0, 4, 8, 12] {
                    let word = input.wrapping_add(offset).cast::<u32>().read_unaligned();
                    output.wrapping_add(offset).cast::<u32>().write_unaligned(word);
                }
            }
            input = input.wrapping_add(16);
            output = output.wrapping_add(16);
        }
        destination.set_end(output);
    }
}

port! {
    /// Copy-constructs `count` vectors from the same source, allocating each record array through the game.
    /// A nonzero count reuses the caller's first argument word as the remaining count, ending at zero.
    #[port(address = 0x004B_0110, owner = "trafficnetwork", name = "fill traffic record vectors", naked)]
    pub unsafe extern "C" fn fc4p_004b0110() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {}", sym fill_vectors_body)
    }
}

unsafe extern "fastcall" fn fill_vectors_body(frame: *mut u32, _edx: u32) -> *mut u8 {
    unsafe {
        let mut destination = frame.add(1).read() as *mut u8;
        let count = frame.add(2).read();
        let source = frame.add(3).read() as *mut u8;
        if count == 0 {
            return destination;
        }
        frame.add(1).write(count);
        loop {
            if !destination.is_null() {
                copy_vector(destination, source, core::ptr::null_mut());
            }
            let remaining = frame.add(1).read().wrapping_sub(1);
            destination = destination.wrapping_add(12);
            frame.add(1).write(remaining);
            if remaining == 0 {
                break;
            }
        }
        destination
    }
}

port! {
    /// Copy-constructs a range of record vectors, returning the end of the constructed destination range.
    /// Each constructed vector leaves its source record end in the caller's destination argument word.
    #[port(address = 0x004B_0440, owner = "trafficnetwork", name = "copy traffic record vectors", naked)]
    pub unsafe extern "C" fn fc4p_004b0440() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {}", sym copy_vectors_body)
    }
}

unsafe extern "fastcall" fn copy_vectors_body(frame: *mut u32, _edx: u32) -> *mut u8 {
    unsafe {
        let mut source = frame.add(1).read() as *mut u8;
        let mut destination = frame.add(3).read() as *mut u8;
        while source != frame.add(2).read() as *mut u8 {
            if !destination.is_null() {
                copy_vector(destination, source, frame.add(3));
            }
            source = source.wrapping_add(12);
            destination = destination.wrapping_add(12);
        }
        destination
    }
}

port! {
    /// Decrements counts above one. Counts zero and one shut down and delete through the current vtable.
    #[port(address = 0x004A_FAA0, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::Release")]
    pub unsafe extern "thiscall" fn fc4p_004afaa0(this: *mut u8) -> u32 {
        unsafe {
            let map = TrafficNetworkMap::from_ptr(this);
            let count = map.references();
            if count > 1 {
                map.set_references(count.wrapping_sub(1));
                return count.wrapping_sub(1);
            }
            vcall!(this, 0x10 => fn());
            vcall!(this, 0xB4 => fn(u32), 1);
            0
        }
    }
}

port! {
    /// Carries entry EDX to the Rust body without changing the one-word thiscall stack.
    #[port(address = 0x004A_FAD0, owner = "trafficnetwork", name = "destroy traffic tree nodes", naked)]
    pub unsafe extern "C" fn fc4p_004afad0() {
        core::arch::naked_asm!("jmp {}", sym destroy_tree_body)
    }
}

// Only EDX is used by callers. Null input preserves it; otherwise it comes from the final free.
unsafe extern "fastcall" fn destroy_tree_body(this: *mut u8, entry_edx: u32, mut node: *mut u8) -> u64 {
    let mut result = u64::from(entry_edx) << 32;
    unsafe {
        while !node.is_null() {
            destroy_tree(this, TreeNode::from_ptr(node).right());
            let next = TreeNode::from_ptr(node).left();
            result = free_node(node);
            node = next;
        }
    }
    result
}

port! {
    /// Releases the network, then every nonnull automaton, then frees all nodes and resets the sentinel links.
    #[port(address = 0x004A_FB10, owner = "trafficnetwork", name = "clear traffic tile objects")]
    pub unsafe extern "thiscall" fn fc4p_004afb10(_this: *mut u8, tile: *mut u8) {
        if tile.is_null() { return; }
        unsafe {
            let tile = TileInfo::from_ptr(tile);
            let network = tile.network();
            if !network.is_null() { vcall!(network, 8 => fn()); }
            let mut header = tile.automata();
            let mut node = AutomatonLink::from_ptr(header).next();
            while node != header {
                let object = AutomatonLink::from_ptr(node).object();
                if !object.is_null() { vcall!(object, 8 => fn()); }
                node = AutomatonLink::from_ptr(node).next();
                header = tile.automata();
            }
            header = tile.automata();
            node = AutomatonLink::from_ptr(header).next();
            while node != header {
                let old = node;
                node = AutomatonLink::from_ptr(node).next();
                free_node(old);
                header = tile.automata();
            }
            header = tile.automata();
            AutomatonLink::from_ptr(header).set_next(header);
            let header = tile.automata();
            AutomatonLink::from_ptr(header).set_previous(header);
        }
    }
}

port! {
    /// Counts matching lanes for automata whose slot 0x154 returns a zero low byte.
    /// AX holds the count; the high half is retained from the last call, or the empty sentinel pointer.
    #[port(address = 0x004B_0000, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::GetAutomataCountForLane")]
    pub unsafe extern "thiscall" fn fc4p_004b0000(this: *mut u8, network: u32, lane: u32) -> u64 {
        unsafe {
            let registers = virtual_words(this, 0x40, &[network], table_word(this), network);
            let tile = registers as u32 as *mut u8;
            if tile.is_null() { return registers; }
            let tile = TileInfo::from_ptr(tile);
            let mut header = tile.automata();
            let mut result = header as u32;
            let mut edx = (registers >> 32) as u32;
            let mut count = 0u32;
            let mut node = AutomatonLink::from_ptr(header).next();
            while node != header {
                let object = AutomatonLink::from_ptr(node).object();
                let registers = virtual_words(object, 0x154, &[], table_word(object), edx);
                result = registers as u32;
                edx = (registers >> 32) as u32;
                if result as u8 == 0 {
                    let registers = virtual_words(object, 0x220, &[], result, table_word(object));
                    result = registers as u32;
                    edx = (registers >> 32) as u32;
                    if result == lane { count = count.wrapping_add(1); }
                }
                node = AutomatonLink::from_ptr(node).next();
                header = tile.automata();
            }
            words((result & 0xFFFF_0000) | (count & 0xFFFF), edx)
        }
    }
}

port! {
    /// Unlinks every matching automaton, decrements nonzero lane and edge counters, and releases each match.
    /// AL reports removal; the other bytes retain the tile address, or entry EAX for a null input.
    #[port(address = 0x004B_02A0, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::ExitNetwork", naked)]
    pub unsafe extern "C" fn fc4p_004b02a0() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {}",
            "add esp, 8", "ret 12", sym exit_body)
    }
}

unsafe extern "fastcall" fn exit_body(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let entry_eax = frame.read();
        let entry_edx = frame.add(1).read();
        let automaton = frame.add(3).read() as *mut u8;
        let network = frame.add(4).read();
        let lane = frame.add(5).read();
        if automaton.is_null() || network == 0 {
            return words(entry_eax & !0xFF, entry_edx);
        }
        let registers = virtual_words(this, 0x40, &[network], table_word(this), entry_edx);
        let tile_ptr = registers as u32 as *mut u8;
        frame.add(4).write(tile_ptr as u32);
        let mut edx = (registers >> 32) as u32;
        if tile_ptr.is_null() {
            return registers;
        }
        let tile = TileInfo::from_ptr(tile_ptr);
        let mut header = tile.automata();
        let mut node = AutomatonLink::from_ptr(header).next();
        let mut removed = false;
        while node != header {
            let link = AutomatonLink::from_ptr(node);
            if link.object() == automaton {
                let next = link.next();
                let previous = link.previous();
                AutomatonLink::from_ptr(previous).set_next(next);
                AutomatonLink::from_ptr(next).set_previous(previous);
                free_node(node);
                node = next;
                let category = Automaton::from_ptr(automaton).category();
                let registers = virtual_words(this, 0x5C, &[network, category, lane], category, table_word(this));
                let counter = registers as u32 as *mut u16;
                let value = counter.read_unaligned();
                if value != 0 {
                    counter.write_unaligned(value.wrapping_sub(1));
                }
                let registers = virtual_words(
                    this,
                    0x58,
                    &[network, (lane >> 24) & 15],
                    table_word(this),
                    (registers >> 32) as u32,
                );
                let counter = registers as u32 as *mut u16;
                let value = counter.read_unaligned();
                if value != 0 {
                    counter.write_unaligned(value.wrapping_sub(1));
                }
                let registers = virtual_words(automaton, 8, &[], counter as u32, table_word(automaton));
                edx = (registers >> 32) as u32;
                removed = true;
            } else {
                node = link.next();
            }
            header = tile.automata();
        }
        words((tile_ptr as u32 & !0xFF) | u32::from(removed), edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_004afaa0::PORT,
    fc4p_004afad0::PORT,
    fc4p_004afb10::PORT,
    fc4p_004b0000::PORT,
    fc4p_004b02a0::PORT,
    fc4p_004b0110::PORT,
    fc4p_004b0440::PORT,
];

#[cfg(test)]
mod tests;
