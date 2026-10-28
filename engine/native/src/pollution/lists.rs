// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Occupant-list property updates and circular-list operations.

use super::*;

game_fn! { fn property_u32(holder: *mut u8, id: u32, output: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn allocate_node(list: *mut u8, count: u32, hint: u32) -> *mut u8 = "thiscall" 0x0041_3DD0; }
game_fn! { fn erase_node(list: *mut u8, output: *mut *mut u8, node: *mut u8) -> u32 = "thiscall" 0x0041_3F70; }
game_fn! { fn erase_range(list: *mut u8, output: *mut *mut u8, first: *mut u8, last: *mut u8) -> u32 = "thiscall" 0x006E_41F0; }
game_fn! { fn append_range(list: *mut u8, position: *mut u8, first: *mut u8, last: *mut u8, context: *mut *mut u8) = "thiscall" 0x006E_4270; }
game_fn! { fn application() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_service(reference: *mut *mut u8) = "thiscall" 0x0041_1870; }
game_data! { static allocator_service: *mut u8 = 0x00B2_07E4; }

port! {
    /// Copies each present used-capacity property into an optional history property, then resets it to zero.
    /// Missing used properties skip both operations. Each occupant supplies its property holder anew.
    #[port(address = 0x006E_3C80, owner = "pollution", name = "cSC4PollutionSimulator::ResetOccupantListCapacityUsedProperty")]
    pub unsafe extern "thiscall" fn fc4p_006e3c80(_this: *mut u8, list: *mut *mut u8, used: u32, previous: u32) {
        // SAFETY: live circular occupant list and its original property/variant interfaces.
        unsafe {
            let sentinel = read_unaligned(list);
            let mut node = ListNode::from_ptr(sentinel).next();
            while node != sentinel {
                let occupant = ListNode::from_ptr(node).occupant();
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, used);
                if !property.is_null() {
                    let value = vcall!(property, 0x14 => fn() -> *mut u8);
                    let old_property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, previous);
                    if !old_property.is_null() {
                        let old_value = vcall!(old_property, 0x14 => fn() -> *mut u8);
                        let current = vcall!(value, 0x12C => fn() -> *const u32);
                        vcall!(old_value, 0x68 => fn(u32), read_unaligned(current));
                    }
                    vcall!(value, 0x74 => fn(u32), 0);
                }
                node = ListNode::from_ptr(node).next();
            }
        }
    }
}

port! {
    /// Writes waste-to-energy usage as the wrapping product of used capacity and output, divided unsigned by capacity.
    /// A missing property stops the remaining property queries for that occupant.
    #[port(address = 0x006E_3F50, owner = "pollution", name = "cSC4PollutionSimulator::ComputeWasteToEnergyListCurrentCapacity")]
    pub unsafe extern "thiscall" fn fc4p_006e3f50(this: *mut u8) {
        // SAFETY: the live energy list, property holders and complete successful property reads.
        unsafe {
            let sentinel = PollutionSim::from_ptr(this).energy_list();
            let mut node = ListNode::from_ptr(sentinel).next();
            while node != sentinel {
                let occupant = ListNode::from_ptr(node).occupant();
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let (mut used, mut capacity, mut output) = (0u32, 0u32, 0u32);
                if property_u32(holder, 0x08FD_1613, &mut used) as u8 != 0
                    && property_u32(holder, 0x48EB_D0B1, &mut capacity) as u8 != 0
                    && property_u32(holder, 0x88EC_F76C, &mut output) as u8 != 0 {
                    let value = unsigned_divide(output.wrapping_mul(used), capacity);
                    vcall!(holder, 0x34 => fn(u32, u32, u32), 0x2781_2852, value, 0);
                }
                node = ListNode::from_ptr(node).next();
            }
        }
    }
}

/// The machine's unsigned division also retains its divide-error behavior for a zero divisor.
fn unsigned_divide(numerator: u32, denominator: u32) -> u32 {
    let quotient: u32;
    // SAFETY: valid operands divide normally; zero raises the original hardware exception.
    unsafe {
        core::arch::asm!("xor edx, edx", "div {denominator:e}", denominator = in(reg) denominator,
            inlateout("eax") numerator => quotient, out("edx") _, options(nomem, nostack));
    }
    quotient
}

port! {
    /// Retains each inserted occupant and inserts a node before the first ordinary node, allowing duplicates.
    /// Removal erases the first matching node before releasing its occupant. Only the low boolean byte is used.
    #[port(address = 0x006E_45F0, owner = "pollution", name = "cSC4PollutionSimulator::AddOrRemoveOccupant")]
    pub unsafe extern "thiscall" fn fc4p_006e45f0(_this: *mut u8, list: *mut *mut u8, occupant: *mut u8, insert: u32) -> u32 {
        // SAFETY: the original live circular list and occupant, with the game's own node allocator and eraser.
        unsafe {
            if insert as u8 != 0 {
                vcall!(occupant, 4 => fn() -> u32);
                let first = ListNode::from_ptr(read_unaligned(list)).next();
                let node = allocate_node(list.cast(), 1, 0);
                let value = node.wrapping_add(8);
                if !value.is_null() { write_unaligned(value.cast(), occupant); }
                let previous = ListNode::from_ptr(first).previous();
                ListNode::from_ptr(node).set_next(first);
                ListNode::from_ptr(node).set_previous(previous);
                ListNode::from_ptr(previous).set_next(node);
                ListNode::from_ptr(first).set_previous(node);
                return (node as u32 & !0xFF) | 1;
            }
            let sentinel = read_unaligned(list);
            let mut node = ListNode::from_ptr(sentinel).next();
            while node != sentinel && ListNode::from_ptr(node).occupant() != occupant {
                node = ListNode::from_ptr(node).next();
            }
            if node == sentinel { return node as u32 & !0xFF; }
            let mut next = core::ptr::null_mut();
            erase_node(list.cast(), &mut next, node);
            let result = vcall!(occupant, 8 => fn() -> u32);
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Unlinks and frees the half-open node range, preserving each next link before calling the allocator service.
    /// The shared service is acquired lazily; output receives the end iterator even for an empty range.
    #[port(address = 0x006E_41F0, owner = "pollution", name = "erase occupant node range")]
    pub unsafe extern "thiscall" fn fc4p_006e41f0(_this: *mut u8, output: *mut *mut u8, first: *mut u8, last: *mut u8) -> *mut *mut u8 {
        // SAFETY: the range contains live list nodes and the allocator interfaces follow their game layout.
        unsafe {
            let mut node = first;
            while node != last {
                let current = ListNode::from_ptr(node);
                let previous = current.previous();
                let next = current.next();
                node = next;
                ListNode::from_ptr(previous).set_next(next);
                ListNode::from_ptr(next).set_previous(previous);
                if (*allocator_service()).is_null() {
                    let mut service = core::ptr::null_mut();
                    let app = application();
                    if !app.is_null() {
                        vcall!(app, 0x14 => fn(u32, u32, *mut *mut u8) -> u32, 0x3AE4_BEA3, 0x3AE4_BEAB, &mut service);
                    }
                    *allocator_service() = service;
                    release_service(&mut service);
                }
                vcall!(*allocator_service(), 0x10 => fn(*mut u8), current.as_ptr());
            }
            write_unaligned(output, last);
            output
        }
    }
}

port! {
    /// Assigns occupant pointers without reference-count changes, reusing existing nodes before erasing or appending.
    /// Assignment to the same list does nothing; the original range helpers handle unequal list lengths.
    #[port(address = 0x006E_5660, owner = "pollution", name = "assign occupant node list")]
    pub unsafe extern "thiscall" fn fc4p_006e5660(this: *mut u8, source: *mut *mut u8) -> *mut u8 {
        // SAFETY: both arguments address live circular list headers.
        unsafe {
            if this == source.cast() { return this; }
            let end = read_unaligned(this.cast::<*mut u8>());
            let source_end = read_unaligned(source);
            let mut destination = ListNode::from_ptr(end).next();
            let mut current = ListNode::from_ptr(source_end).next();
            while destination != end && current != source_end {
                let value = ListNode::from_ptr(current).occupant();
                current = ListNode::from_ptr(current).next();
                let previous = destination;
                destination = ListNode::from_ptr(destination).next();
                ListNode::from_ptr(previous).set_occupant(value);
            }
            let mut context = source.cast::<u8>();
            if current == source_end {
                erase_range(this, &mut context, destination, end);
            } else {
                append_range(this, end, current, source_end, &mut context);
            }
            this
        }
    }
}

port! {
    /// Distributes a fixed initial share across occupants. A shortfall recomputes the next share before
    /// subtracting this occupant's allocation. The comparison of share with available capacity is signed;
    /// capacity differences, remaining counts and remaining capacity otherwise wrap as unsigned words.
    #[port(address = 0x006E_3FE0, owner = "pollution", name = "cSC4PollutionSimulator::SetOccupantListUsedCapacity")]
    pub unsafe extern "thiscall" fn fc4p_006e3fe0(
        this: *mut u8, list: *mut *mut u8, capacity_property: u32, used_property: u32,
        total: u32, remaining: *mut u32, count: u32,
    ) {
        if total == 0 || count == 0 { return; }
        // SAFETY: the live list, remaining-capacity word, and original property/variant interfaces.
        unsafe {
            let mut share = unsigned_divide(total.min(read_unaligned(remaining)), count);
            let sentinel = read_unaligned(list);
            let mut node = ListNode::from_ptr(sentinel).next();
            let mut count = count;
            while read_unaligned(remaining) != 0 && node != sentinel {
                let occupant = ListNode::from_ptr(node).occupant();
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let occupant = ListNode::from_ptr(node).occupant();
                count = count.wrapping_sub(1);
                let mut capacity = 0u32;
                let result = vcall!(this, 0xD4 => fn(u32, *mut u8, *mut u32) -> u32, capacity_property, occupant, &mut capacity);
                if result as u8 != 0 {
                    let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, used_property);
                    if !property.is_null() {
                        let variant = vcall!(property, 0x14 => fn() -> *mut u8);
                        let pointer = vcall!(variant, 0x12C => fn() -> *const u32);
                        let used = read_unaligned(pointer);
                        let available = if capacity > used { capacity.wrapping_sub(used) } else { 0 };
                        let allocated = if (share as i32) > (available as i32) {
                            if count != 0 { share = unsigned_divide(read_unaligned(remaining), count); }
                            available
                        } else { share };
                        write_unaligned(remaining, read_unaligned(remaining).wrapping_sub(allocated));
                        vcall!(variant, 0x74 => fn(u32), used.wrapping_add(allocated));
                    }
                }
                node = ListNode::from_ptr(node).next();
            }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006e3c80::PORT,
    fc4p_006e3fe0::PORT,
    fc4p_006e3f50::PORT,
    fc4p_006e41f0::PORT,
    fc4p_006e45f0::PORT,
    fc4p_006e5660::PORT,
];

#[cfg(test)]
mod tests;
