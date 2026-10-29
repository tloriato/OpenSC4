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

//! Shared helpers, group 04: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { #[allow(dead_code)] fn destroy_vehicle(this: *mut u8, flags: u32) -> u32 = "thiscall" 0x0053_CCE0; }
game_fn! { #[allow(dead_code)] fn retain_dispatch(this: *mut u8) -> u32 = "thiscall" 0x004B_D280; }
game_fn! { #[allow(dead_code)] fn release_dispatch(this: *mut u8) -> u32 = "thiscall" 0x004B_D290; }
game_fn! { #[allow(dead_code)] fn retain_effect(this: *mut u8) -> u32 = "thiscall" 0x0057_EB00; }
game_fn! { #[allow(dead_code)] fn release_effect(this: *mut u8) -> u32 = "thiscall" 0x0057_EB70; }
game_fn! { #[allow(dead_code)] fn allocate_with_tag(size: u32, tag: u32) -> u64 = "C" 0x0090_D91C; }
game_fn! { fn free_vehicle_pool(this: *mut u8, storage: *mut u8) -> u32 = "thiscall" 0x0055_F600; }
game_fn! { fn free_vehicle(storage: *mut u8) -> u32 = "C" 0x005E_5620; }
game_fn! { fn free(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn grow_deque_map(this: *mut u8, count: u32, front: u32) = "thiscall" 0x006D_B4A0; }
game_fn! { fn append_deque_block(this: *mut u8, item: *const u8) -> u32 = "thiscall" 0x0056_0250; }
game_fn! { fn insert_vector(this: *mut u8, position: *mut u8, value: *const u8, guard: *mut u8, count: u32, capacity: u32) -> u32 = "thiscall" 0x0043_0A90; }
game_data! { static vehicle_pool: u8 = 0x00B0_A0F8; }
game_fn! { fn allocate_pool_block(size: u32) -> *mut u8 = "C" 0x009F_066E; }
game_fn! { fn grow_pool(this: *mut u8) -> u64 = "thiscall" 0x005A_61B0; }
game_data! { static default_coordinate: u32 = 0x00B2_2A80; }
game_data! { static proximity_scale: f32 = 0x00B0_A158; }
game_data! { static initial_path_coordinate: u32 = 0x00B2_293C; }
game_data! { static path_unit: f32 = 0x00B0_A07C; }
game_data! { static path_epsilon: f32 = 0x00A9_BCD8; }
game_data! { static path_zero: f32 = 0x00A8_1054; }
game_data! { static route_city: *mut u8 = 0x00B4_3CEC; }
game_fn! { fn byte_key_bucket(this: *mut u8, key: *const u8) -> u32 = "thiscall" 0x0051_C9B0; }
game_fn! { fn assign_history_reference(this: *mut u8, source: *mut u8) = "thiscall" 0x0054_DB10; }
game_fn! { fn last_history_entry(this: *mut u8) -> *mut u8 = "thiscall" 0x0055_F900; }
game_fn! { fn pop_history_back(this: *mut u8) = "thiscall" 0x0055_FA90; }
game_fn! { fn append_history_entry(this: *mut u8, value: *const u8) = "thiscall" 0x0056_14B0; }
game_fn! { fn pop_history_front(this: *mut u8) = "thiscall" 0x0055_FAF0; }
game_fn! { fn pop_route_front(this: *mut u8) -> u64 = "thiscall" 0x0055_3720; }
game_fn! { fn route_begin(this: *mut u8, output: *mut u32) -> *mut u32 = "thiscall" 0x006C_5FE0; }
game_fn! { fn advance_route_iterator(this: *mut u32) = "thiscall" 0x0050_4C40; }
game_fn! { fn assign_route_network(this: *mut u8, network: *mut u8) -> u64 = "thiscall" 0x0054_47B0; }

port! {
    /// Consumes an explicit route cell and chooses an accepting network and exit for the next node.
    /// The entry direction occupies one byte of an otherwise retained stack word. Exit and attempt
    /// bytes replace the low bytes of the caller's two argument words.
    #[port(address = 0x0055_27E0, owner = "shared", name = "ExpandExplicitRoute", naked)]
    pub unsafe extern "C" fn fc4p_005527e0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 4]", "push dword ptr [edx - 12]",
            "call {body}", "ret 8", body = sym expand_explicit_route);
    }
}

unsafe extern "fastcall" fn expand_explicit_route(
    this: *mut u8,
    stack: *mut u32,
    entry_word: u32,
    entry_edx: u32,
) -> u64 {
    unsafe {
        let city = read_unaligned(route_city());
        let manager = virtual_integer(city, 0xA0, pointer(city, 0) as u32, entry_edx, &[]).0;
        let manager = manager as *mut u8;
        let previous = read_unaligned(stack.add(1)) as *mut u8;
        let word = read_unaligned(previous.add(4).cast::<u32>());
        let selector = (word >> 24) & 15;
        let entry = (word as u8).wrapping_add(2) & 3;
        let entry_word = (entry_word & !0xFF) | u32::from(entry);
        let front = pointer(this, 0x22C);
        let mut edx = pointer(this, 0x23C) as u32;
        if edx == front as u32 {
            return u64::from(edx) << 32;
        }
        let x = u32::from(read_unaligned(front));
        let y = u32::from(read_unaligned(front.add(1)));
        let (mut candidate, returned_edx) = virtual_integer(manager, 0x38, y, pointer(manager, 0) as u32, &[x, y]);
        edx = returned_edx;
        if pointer(this, 0x22C) == pointer(this, 0x234).wrapping_sub(3) {
            let block = pointer(this, 0x230);
            if !block.is_null() {
                free(block);
            }
            let next_map = pointer(this, 0x238).wrapping_add(4);
            put_pointer(this, 0x238, next_map);
            let next = pointer(next_map, 0);
            put_pointer(this, 0x230, next);
            edx = pointer(this, 0x230) as u32;
            put_pointer(this, 0x234, next.wrapping_add(126));
            put_pointer(this, 0x22C, edx as *mut u8);
        } else {
            put_pointer(this, 0x22C, pointer(this, 0x22C).wrapping_add(3));
        }
        let next = pointer(this, 0x22C);
        let output = read_unaligned(stack.add(2)) as *mut u8;
        if pointer(this, 0x23C) != next {
            let dy = i32::from(read_unaligned(next.add(1))).wrapping_sub(y as i32);
            let exit = if dy < 0 {
                1
            } else if dy > 0 {
                3
            } else if u32::from(read_unaligned(next)) < x {
                0
            } else {
                2
            };
            let exit_word = (read_unaligned(stack.add(1)) & !0xFF) | exit;
            write_unaligned(stack.add(1).cast::<u8>(), exit as u8);
            while candidate != 0 {
                let node = candidate as *mut u8;
                let network = pointer(node, 0);
                if !network.is_null() {
                    let result = virtual_integer(
                        network,
                        0xD4,
                        pointer(network, 0) as u32,
                        candidate,
                        &[selector, entry_word, exit_word],
                    );
                    if result.0 as u8 != 0 {
                        let output = read_unaligned(stack.add(2)) as *mut u8;
                        let args = [network as u32];
                        let mut call = crate::abi::RawCall::new(
                            crate::abi::Conv::Thiscall,
                            crate::abi::target(0x0054_47B0),
                            output as u32,
                            &args,
                        );
                        call.eax = result.0;
                        call.edx = result.1;
                        crate::abi::call_raw(&mut call);
                        write_unaligned(
                            output.add(4).cast::<u32>(),
                            (selector << 24) | (u32::from(entry) << 8) | exit,
                        );
                        write_unaligned(output.add(8), 0);
                        return (u64::from(call.edx) << 32) | 1;
                    }
                }
                edx = candidate;
                candidate = pointer(node, 0x54) as u32;
            }
            return u64::from(edx) << 32;
        }
        write_unaligned(output.add(8), 1);
        let previous = read_unaligned(stack.add(1)) as *mut u8;
        let args = [pointer(previous, 0) as u32, word, 1, selector];
        let (found, returned_edx) = virtual_integer(manager, 0x3C, args[0], pointer(manager, 0) as u32, &args);
        edx = returned_edx;
        if found == 0 {
            return u64::from(edx) << 32;
        }
        assign_route_network(output, pointer(found as *mut u8, 0));
        let mut exit = u32::from(entry).wrapping_add(2) & 3;
        write_unaligned(stack.add(1).cast::<u8>(), exit as u8);
        write_unaligned(stack.add(2).cast::<u8>(), 0);
        let mut eax;
        for attempt in 0..4 {
            let network = pointer(output, 0);
            (eax, edx) = virtual_integer(
                network,
                0xD4,
                entry_word,
                pointer(network, 0) as u32,
                &[selector, entry_word, read_unaligned(stack.add(1))],
            );
            if eax as u8 != 0 {
                write_unaligned(
                    output.add(4).cast::<u32>(),
                    (selector << 24) | (u32::from(entry) << 8) | exit,
                );
                return (u64::from(exit) << 32) | u64::from((eax & !0xFF) | 1);
            }
            exit = (exit + 1) & 3;
            write_unaligned(stack.add(1).cast::<u8>(), exit as u8);
            write_unaligned(stack.add(2).cast::<u8>(), (attempt + 1) as u8);
            if attempt == 3 {
                return (u64::from(edx) << 32) | u64::from(eax & !0xFF);
            }
        }
        core::hint::unreachable_unchecked()
    }
}

unsafe fn virtual_integer(object: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> (u32, u32) {
    unsafe {
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(object, slot),
            object as u32,
            args,
        );
        call.eax = eax;
        call.edx = edx;
        crate::abi::call_raw(&mut call);
        (call.eax, call.edx)
    }
}

port! {
    /// Consumes the current look-ahead node and rebuilds the remaining path identifiers in order.
    /// Successful geometry queries carry their final position into the next query. Terminal entries stop
    /// movement or trigger arrival, and a nonzero retry byte decreases once after rebuilding.
    #[port(address = 0x0055_3AA0, owner = "shared", name = "RebuildLookAheadPaths")]
    pub unsafe extern "fastcall" fn fc4p_00553aa0(this: *mut u8, entry_edx: u32) -> u32 {
        unsafe {
            let mut eax = virtual_integer(this, 0x2B8, pointer(this, 0) as u32, entry_edx, &[]).0;
            if eax == 0 { return 0; }
            eax = pointer(this, 0x1EC) as u32;
            if eax == 0 { return 0; }
            let (result, mut edx) = virtual_integer(this, 0x7C, eax, pointer(this, 0) as u32, &[0x2000]);
            eax = result;
            if eax as u8 != 0 { return eax & !0xFF; }
            virtual_integer(this, 0x298, pointer(this, 0) as u32, edx, &[]);
            let deque = this.add(0x204);
            let first = pointer(deque, 0);
            if pointer(deque, 0x10) == first {
                return virtual_integer(this, 0x34, first as u32, pointer(this, 0) as u32, &[0]).0 & !0xFF;
            }
            let object = pointer(first, 0);
            if !object.is_null() {
                put_pointer(first, 0, core::ptr::null_mut());
                vcall!(object, 8 => fn());
            }
            if read_unaligned(first.add(8)) != 0 {
                vcall!(this, 0x34 => fn(u32), 0);
                return pop_route_front(deque) as u32 & !0xFF;
            }
            let popped = pop_route_front(deque);
            eax = pointer(deque, 0x10) as u32;
            if eax == pointer(deque, 0) as u32 {
                return virtual_integer(this, 0x34, eax, pointer(this, 0) as u32, &[0]).0 & !0xFF;
            }
            let path = pointer(this, 0x1F8);
            let position = if path.is_null() {
                virtual_integer(this, 0x230, pointer(this, 0) as u32, (popped >> 32) as u32, &[]).0 as *mut u8
            } else {
                pointer(path, 4).wrapping_sub(12)
            };
            let mut point = [read_unaligned(position.cast::<u32>()), read_unaligned(position.add(4).cast()), read_unaligned(position.add(8).cast())];
            let mut initial = [0u32; 4];
            let begin = route_begin(deque, initial.as_mut_ptr());
            let mut iterator = [read_unaligned(begin), read_unaligned(begin.add(1)), read_unaligned(begin.add(2)), read_unaligned(begin.add(3))];
            let mut changed = 0;
            let mut identifier = core::mem::MaybeUninit::<u32>::uninit();
            let mut distance = core::mem::MaybeUninit::<u32>::uninit();
            while iterator[0] != pointer(deque, 0x10) as u32 {
                let node = iterator[0] as *mut u8;
                let args = [iterator[0], iterator[1], iterator[2], iterator[3], point.as_mut_ptr() as u32,
                    identifier.as_mut_ptr() as u32, distance.as_mut_ptr() as u32];
                let accepted = virtual_integer(this, 0x2A4, pointer(this, 0) as u32, iterator[3], &args).0;
                if accepted as u8 != 0 {
                    let id = identifier.assume_init();
                    write_unaligned(node.add(4).cast::<u32>(), id);
                    changed = 1;
                    let network = pointer(node, 0);
                    let service = vcall!(network, 0xC4 => fn() -> *mut u8);
                    if !service.is_null() {
                        let path = vcall!(service, 0x0C => fn(u32, u32) -> *mut u8, identifier.assume_init(), 0);
                        if !path.is_null() {
                            let end = pointer(path, 4).wrapping_sub(12);
                            point[0] = read_unaligned(end.cast());
                            point[1] = read_unaligned(end.add(4).cast());
                            point[2] = read_unaligned(end.add(8).cast());
                        }
                    }
                }
                advance_route_iterator(iterator.as_mut_ptr());
            }
            let retries = read_unaligned(this.add(0x1E8));
            if retries != 0 { write_unaligned(this.add(0x1E8), retries.wrapping_sub(1)); }
            let node = pointer(deque, 0);
            if pointer(deque, 0x10) == node { return (node as u32 & !0xFF) | changed; }
            let args = [pointer(node, 0) as u32, read_unaligned(node.add(4).cast()), 1];
            (eax, edx) = virtual_integer(this, 0x20C, pointer(this, 0) as u32, args[0], &args);
            if read_unaligned(node.add(8)) == 0 { return (eax & !0xFF) | changed; }
            (eax, edx) = virtual_integer(this, 0x160, pointer(this, 0) as u32, edx, &[]);
            if eax as u8 == 0 {
                eax = virtual_integer(this, 0x34, pointer(this, 0) as u32, edx, &[0]).0;
            } else {
                eax = virtual_integer(this, 0x13C, eax, pointer(this, 0) as u32, &[]).0;
            }
            (eax & !0xFF) | changed
        }
    }
}

/// Deque size arithmetic uses signed byte differences and wraps at each integer operation.
fn history_size(deque: EntryDeque) -> (u32, u32) {
    let back = (deque.last() as u32).wrapping_sub(deque.last_block() as u32) as i32 >> 3;
    let blocks = (deque.last_map() as u32).wrapping_sub(deque.first_map() as u32) as i32 >> 2;
    let front = (deque.first_limit() as u32).wrapping_sub(deque.first() as u32) as i32 >> 3;
    (
        back.wrapping_add(blocks.wrapping_sub(1).wrapping_shl(4))
            .wrapping_add(front) as u32,
        front as u32,
    )
}

port! {
    /// Records the current interface and payload in a supplied deque, replacing a matching last entry.
    /// The low byte of the limit controls trimming. Cleared references are visible before release calls.
    /// Both integer return registers retain the last call or size calculation, including early exits.
    #[port(address = 0x0056_1910, owner = "shared", name = "RecordRecentInterface")]
    pub unsafe extern "fastcall" fn fc4p_00561910(this: *mut u8, entry_edx: u32, history: *mut u8, limit: u32) -> u64 {
        unsafe {
            let args = [0x2000];
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, crate::abi::vslot(this, 0x7C), this as u32, &args);
            call.eax = pointer(this, 0) as u32;
            call.edx = entry_edx;
            crate::abi::call_raw(&mut call);
            if call.eax as u8 != 0 {
                return (u64::from(call.edx) << 32) | u64::from(call.eax);
            }
            let source = this.add(0x1EC);
            let current = pointer(source, 0);
            let limit = limit & 0xFF;
            if current.is_null() || limit == 0 {
                return (u64::from(call.edx) << 32) | u64::from(current as u32);
            }
            let mut entry = [0u32, read_unaligned(this.add(0x1F4).cast())];
            assign_history_reference(entry.as_mut_ptr().cast(), source);
            let deque = EntryDeque::from_ptr(history);
            if deque.last() != deque.first() {
                let last = last_history_entry(history);
                let object = pointer(last, 0);
                if object == pointer(source, 0) {
                    if !object.is_null() {
                        put_pointer(last, 0, core::ptr::null_mut());
                        vcall!(object, 8 => fn());
                    }
                    pop_history_back(history);
                }
            }
            append_history_entry(history, entry.as_ptr().cast());
            let (mut size, _) = history_size(deque);
            let mut edx = size;
            while size > limit {
                let first = deque.first();
                let object = pointer(first, 0);
                if !object.is_null() {
                    put_pointer(first, 0, core::ptr::null_mut());
                    vcall!(object, 8 => fn());
                }
                pop_history_front(history);
                (size, edx) = history_size(deque);
            }
            let object = entry[0] as *mut u8;
            let mut eax = deque.first() as u32;
            if !object.is_null() {
                let regs = virtual_registers(object, 8, pointer(object, 0) as u32, edx);
                eax = regs[0];
                edx = regs[2];
            }
            (u64::from(edx) << 32) | u64::from(eax)
        }
    }
}

port! {
    /// Finds the contiguous range of matching byte keys in a bucket chain and its following iterator.
    /// With a nonempty bucket, the key argument is replaced by the key's first-byte pointer.
    #[port(address = 0x0058_6EF0, owner = "shared", name = "FindByteKeyRange", naked)]
    pub unsafe extern "C" fn fc4p_00586ef0() {
        core::arch::naked_asm!("mov edx, esp", "call {body}", "ret 8", body = sym find_byte_key_range);
    }
}

unsafe fn equal_bytes(a: *mut u8, b: *mut u8, count: u32) -> bool {
    unsafe {
        let mut i = 0u32;
        while i != count {
            if read_unaligned(a.wrapping_add(i as usize)) != read_unaligned(b.wrapping_add(i as usize)) {
                return false;
            }
            i = i.wrapping_add(1);
        }
        true
    }
}

unsafe extern "fastcall" fn find_byte_key_range(this: *mut u8, stack: *mut u32) -> *mut u8 {
    unsafe {
        let output = read_unaligned(stack.add(1)) as *mut u8;
        let key = read_unaligned(stack.add(2)) as *mut u8;
        let bucket = byte_key_bucket(this, key);
        let mut node = pointer(pointer(this, 4).wrapping_add(bucket.wrapping_mul(4) as usize), 0);
        let mut first = core::ptr::null_mut();
        let mut after = core::ptr::null_mut();
        if !node.is_null() {
            let begin = pointer(key, 0);
            let length = (pointer(key, 4) as u32).wrapping_sub(begin as u32);
            write_unaligned(stack.add(2), begin as u32);
            loop {
                let candidate = pointer(node, 4);
                let count = (pointer(node, 8) as u32).wrapping_sub(candidate as u32);
                if count == length && equal_bytes(begin, candidate, length) {
                    first = node;
                    break;
                }
                node = pointer(node, 0);
                if node.is_null() {
                    break;
                }
            }
            if !first.is_null() {
                after = pointer(first, 0);
                while !after.is_null() {
                    let candidate = pointer(after, 4);
                    let count = (pointer(after, 8) as u32).wrapping_sub(candidate as u32);
                    if count != length || !equal_bytes(begin, candidate, length) {
                        break;
                    }
                    after = pointer(after, 0);
                }
                if after.is_null() {
                    let mut index = bucket.wrapping_add(1);
                    let count = (pointer(this, 8) as u32).wrapping_sub(pointer(this, 4) as u32) as i32 >> 2;
                    if index < count as u32 {
                        let mut slot = pointer(this, 4).wrapping_add(index.wrapping_mul(4) as usize);
                        loop {
                            if !pointer(slot, 0).is_null() {
                                after = pointer(pointer(this, 4).wrapping_add(index.wrapping_mul(4) as usize), 0);
                                break;
                            }
                            let count = (pointer(this, 8) as u32).wrapping_sub(pointer(this, 4) as u32) as i32 >> 2;
                            index = index.wrapping_add(1);
                            slot = slot.wrapping_add(4);
                            if index >= count as u32 {
                                break;
                            }
                        }
                    }
                }
            }
        }
        if first.is_null() {
            put_pointer(output, 4, this);
            put_pointer(output, 0, first);
        } else {
            put_pointer(output, 0, first);
            put_pointer(output, 4, this);
        }
        put_pointer(output, 8, after);
        put_pointer(output, 12, this);
        output
    }
}

/// A no-argument virtual call with all three caller-visible integer registers.
unsafe fn virtual_registers(object: *mut u8, slot: usize, eax: u32, edx: u32) -> [u32; 3] {
    unsafe {
        let target = crate::abi::vslot(object, slot);
        let (mut eax, mut ecx, mut edx) = (eax, object as u32, edx);
        core::arch::asm!("call {target}", target = in(reg) target,
            inlateout("eax") eax, inlateout("ecx") ecx, inlateout("edx") edx, clobber_abi("C"));
        [eax, ecx, edx]
    }
}

port! {
    /// Copies retained interface pointers forwards. The remaining count replaces the second argument.
    /// The adapter carries ECX and EDX through skipped assignments and virtual calls.
    #[port(address = 0x0056_8BA0, owner = "shared", name = "CopyInterfacePointers", naked)]
    pub unsafe extern "C" fn fc4p_00568ba0() {
        core::arch::naked_asm!("push edx", "push ecx", "sub esp, 12", "lea ecx, [esp + 20]",
            "mov edx, esp", "call {body}", "mov eax, [esp]", "mov ecx, [esp + 4]",
            "mov edx, [esp + 8]", "add esp, 20", "ret", body = sym copy_interfaces);
    }
}

unsafe extern "fastcall" fn copy_interfaces(stack: *mut u32, registers: *mut [u32; 3]) {
    unsafe {
        let mut source = read_unaligned(stack.add(1)) as *mut u8;
        let end = read_unaligned(stack.add(2));
        let mut destination = read_unaligned(stack.add(3)) as *mut u8;
        let mut regs = [0, read_unaligned(stack.sub(2)), read_unaligned(stack.sub(1))];
        let count = end.wrapping_sub(source as u32) as i32 >> 2;
        if count > 0 {
            regs[0] = count as u32;
            write_unaligned(stack.add(2), count as u32);
            loop {
                if source != destination {
                    let new = pointer(source, 0);
                    let old = pointer(destination, 0);
                    if new != old {
                        if !new.is_null() {
                            regs = virtual_registers(new, 4, pointer(new, 0) as u32, regs[2]);
                        }
                        put_pointer(destination, 0, new);
                        if !old.is_null() {
                            regs = virtual_registers(old, 8, regs[0], pointer(old, 0) as u32);
                        }
                    }
                }
                let count = read_unaligned(stack.add(2)).wrapping_sub(1);
                source = source.wrapping_add(4);
                destination = destination.wrapping_add(4);
                write_unaligned(stack.add(2), count);
                regs[0] = count;
                if count == 0 {
                    break;
                }
            }
        }
        regs[0] = destination as u32;
        registers.write(regs);
    }
}

// Keep stored operands in memory so arithmetic preserves signaling-NaN operand roles.
macro_rules! stored_float_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = F80::ZERO;
            unsafe { core::arch::asm!("fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                out("st(0)") _); }
            output
        }
    };
}
stored_float_op!(subtract_float, "fsub dword ptr [{operand}]");
stored_float_op!(multiply_float, "fmul dword ptr [{operand}]");
stored_float_op!(add_float, "fadd dword ptr [{operand}]");
stored_float_op!(divide_from_float, "fdivr dword ptr [{operand}]");

fn compare_float(value: F80, operand: f32) -> u16 {
    let status;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") status, out("st(0)") _);
    }
    status
}

/// Adds the three displacements in write order; only the z product is stored before addition.
unsafe fn displace_path_point(point: *mut f32, direction: [f32; 3], distance: f32) {
    unsafe {
        let x = multiply_float(F80::from_f32(direction[0]), distance);
        let y = multiply_float(F80::from_f32(direction[1]), distance);
        let z = multiply_float(F80::from_f32(direction[2]), distance).to_f32();
        write_unaligned(point, add_float(x, read_unaligned(point)).to_f32());
        write_unaligned(point.add(1), add_float(y, read_unaligned(point.add(1))).to_f32());
        write_unaligned(
            point.add(2),
            add_float(F80::from_f32(z), read_unaligned(point.add(2))).to_f32(),
        );
    }
}

port! {
    /// Walks a distance along transformed path points, extrapolating beyond the final segment.
    /// The remaining distance, last point index and segment length replace their original argument words.
    #[port(address = 0x0055_04E0, owner = "shared", name = "AdvanceAlongPath", naked)]
    pub unsafe extern "C" fn fc4p_005504e0() {
        core::arch::naked_asm!("mov edx, esp", "call {body}", "ret 24", body = sym advance_along_path);
    }
}

unsafe extern "fastcall" fn advance_along_path(this: *mut u8, stack: *mut u32) -> u64 {
    unsafe {
        let mut edx = read_unaligned(initial_path_coordinate());
        let overflow = read_unaligned(stack.add(6)) as *mut f32;
        if !overflow.is_null() {
            write_unaligned(overflow, 0.0);
        }
        let mut vector = read_unaligned(stack.add(5)) as *mut u8;
        if vector.is_null() {
            vector = pointer(this, 0x1F8);
            if vector.is_null() {
                return (u64::from(edx) << 32) | u64::from(overflow as u32 & !0xFF);
            }
        }
        let mut index = read_unaligned(stack.add(4)) as *mut u32;
        if index.is_null() {
            index = this.add(0x1FC).cast();
        }
        let count = (pointer(vector, 4) as u32).wrapping_sub(pointer(vector, 0) as u32) as i32 / 12;
        let source = read_unaligned(stack.add(2)) as *mut u32;
        let output = read_unaligned(stack.add(3)) as *mut f32;
        write_unaligned(output.cast::<u32>(), read_unaligned(source));
        edx = read_unaligned(source.add(1));
        write_unaligned(output.add(1).cast::<u32>(), edx);
        write_unaligned(output.add(2).cast::<u32>(), read_unaligned(source.add(2)));
        let last = count.wrapping_sub(1) as u32;
        if (read_unaligned(index) as i32) < 0 {
            write_unaligned(index, 0);
        } else {
            edx = last;
            if read_unaligned(index) >= last {
                let overflow = read_unaligned(stack.add(6)) as *mut f32;
                if !overflow.is_null() {
                    write_unaligned(overflow.cast::<u32>(), read_unaligned(stack.add(1)));
                }
                return (u64::from(edx) << 32) | u64::from(overflow as u32 & !0xFF);
            }
        }
        write_unaligned(stack.add(4), last);
        loop {
            let at = read_unaligned(index);
            if at >= last {
                return (u64::from(edx) << 32) | u64::from(at & !0xFF);
            }
            let next = pointer(vector, 0).wrapping_add(at.wrapping_mul(3).wrapping_add(3).wrapping_mul(4) as usize);
            let mut target = [
                read_unaligned(next.cast::<u32>()),
                read_unaligned(next.add(4).cast()),
                read_unaligned(next.add(8).cast()),
            ];
            let args = [target.as_mut_ptr() as u32];
            let mut call = crate::abi::RawCall::new(
                crate::abi::Conv::Thiscall,
                crate::abi::vslot(this, 0x274),
                this as u32,
                &args,
            );
            call.eax = target.as_mut_ptr() as u32;
            call.edx = pointer(this, 0) as u32;
            crate::abi::call_raw(&mut call);
            edx = call.edx;
            let x = subtract_float(F80::from_f32(f32::from_bits(target[0])), read_unaligned(output));
            let y = subtract_float(F80::from_f32(f32::from_bits(target[1])), read_unaligned(output.add(1))).to_f32();
            let z = subtract_float(F80::from_f32(f32::from_bits(target[2])), read_unaligned(output.add(2)));
            let mut direction = [x.to_f32(), y, z.to_f32()];
            let yy = multiply_float(F80::from_f32(y), y);
            let distance = add_popped(add_popped(z * z, yy), x * x).sqrt();
            let stored_distance = distance.to_f32();
            write_unaligned(stack.add(5), stored_distance.to_bits());
            if compare_float(F80::from_f32(stored_distance), read_unaligned(path_epsilon())) & 0x4100 == 0 {
                let inverse = divide_from_float(distance, read_unaligned(path_unit()));
                direction[0] = (x * inverse).to_f32();
                direction[1] = (F80::from_f32(y) * inverse).to_f32();
                direction[2] = (z * inverse).to_f32();
            }
            let remaining = f32::from_bits(read_unaligned(stack.add(1)));
            let status = compare_float(F80::from_f32(f32::from_bits(read_unaligned(stack.add(5)))), remaining);
            let mut eax = (call.eax & 0xFFFF_0000) | u32::from(status);
            if status & 0x100 == 0 {
                displace_path_point(output, direction, remaining);
                return (u64::from(edx) << 32) | u64::from((eax & !0xFF) | 1);
            }
            let next_index = read_unaligned(index).wrapping_add(1);
            write_unaligned(index, next_index);
            edx = last;
            if next_index < last {
                let remaining =
                    subtract_float(F80::from_f32(remaining), f32::from_bits(read_unaligned(stack.add(5)))).to_f32();
                write_unaligned(output.cast::<u32>(), target[0]);
                write_unaligned(stack.add(1), remaining.to_bits());
                write_unaligned(output.add(1).cast::<u32>(), target[1]);
                edx = target[2];
                write_unaligned(output.add(2).cast::<u32>(), edx);
                continue;
            }
            let status = compare_float(
                F80::from_f32(f32::from_bits(read_unaligned(stack.add(5)))),
                read_unaligned(path_zero()),
            );
            eax = (next_index & 0xFFFF_0000) | u32::from(status);
            if status & 0x4100 == 0 {
                let remaining = f32::from_bits(read_unaligned(stack.add(1)));
                let status = compare_float(F80::from_f32(remaining), read_unaligned(path_zero()));
                eax = (eax & 0xFFFF_0000) | u32::from(status);
                if status & 0x4100 == 0 {
                    let overflow = read_unaligned(stack.add(6)) as *mut f32;
                    eax = overflow as u32;
                    if !overflow.is_null() {
                        write_unaligned(
                            overflow,
                            subtract_float(F80::from_f32(remaining), f32::from_bits(read_unaligned(stack.add(5))))
                                .to_f32(),
                        );
                    }
                    displace_path_point(output, direction, remaining);
                }
            }
            return (u64::from(edx) << 32) | u64::from(eax & !0xFF);
        }
    }
}

fn add_popped(older: F80, newer: F80) -> F80 {
    let mut output = F80::ZERO;
    unsafe {
        core::arch::asm!("fld tbyte ptr [{older}]", "fld tbyte ptr [{newer}]", "faddp st(1), st(0)",
        "fstp tbyte ptr [{output}]", older = in(reg) &older, newer = in(reg) &newer,
        output = in(reg) &mut output, out("st(0)") _, out("st(1)") _);
    }
    output
}

port! {
    /// Finds the first other object whose scaled radius strictly exceeds its distance to the point.
    /// Each tested distance replaces the fourth argument word, including when the radius rejects it.
    #[port(address = 0x0056_8C10, owner = "shared", name = "FindObjectNearPoint", naked)]
    pub unsafe extern "C" fn fc4p_00568c10() {
        core::arch::naked_asm!("mov edx, esp", "call {body}", "ret 16", body = sym find_near_point);
    }
}

unsafe extern "fastcall" fn find_near_point(this: *mut u8, stack: *mut u32) -> *mut u8 {
    unsafe {
        let header = pointer(this, 0x44);
        let mut node = pointer(header, 0);
        if node == header {
            return core::ptr::null_mut();
        }
        let ignore = read_unaligned(stack.add(4)) as *mut u8;
        loop {
            let object = pointer(node, 8);
            if !object.is_null() {
                let coordinate = read_unaligned(default_coordinate());
                let mut position = [coordinate; 3];
                if object != ignore {
                    vcall!(object, 0x1C => fn(*mut u32), position.as_mut_ptr());
                    let dx = subtract_float(
                        F80::from_f32(f32::from_bits(read_unaligned(stack.add(1)))),
                        f32::from_bits(position[0]),
                    );
                    let dy = subtract_float(
                        F80::from_f32(f32::from_bits(read_unaligned(stack.add(2)))),
                        f32::from_bits(position[1]),
                    );
                    let dz = subtract_float(
                        F80::from_f32(f32::from_bits(read_unaligned(stack.add(3)))),
                        f32::from_bits(position[2]),
                    );
                    let sum = add_popped(add_popped(dz * dz, dy * dy), dx * dx);
                    let distance = sum.sqrt().to_f32();
                    write_unaligned(stack.add(4), distance.to_bits());
                    let radius = vcall_st0!(object, 0x44);
                    let scaled = multiply_float(radius, read_unaligned(proximity_scale()));
                    if scaled
                        .compare(F80::from_f32(f32::from_bits(read_unaligned(stack.add(4)))))
                        .status_word()
                        & 0x4100
                        == 0
                    {
                        return object;
                    }
                }
            }
            node = pointer(node, 0);
            if node == pointer(this, 0x44) {
                return core::ptr::null_mut();
            }
        }
    }
}

port! {
    /// Destroys nodes with two owned arrays and two retained interfaces, then resets both sentinel links.
    #[port(address = 0x0057_0C50, owner = "shared", name = "ClearOwnedRouteList")]
    pub unsafe extern "thiscall" fn fc4p_00570c50(this: *mut u8) -> *mut u8 {
        unsafe {
            let mut header = pointer(this, 0);
            let mut node = pointer(header, 0);
            while node != header {
                let current = node;
                let storage = pointer(current, 0x30);
                node = pointer(current, 0);
                if !storage.is_null() { free(storage); }
                let storage = pointer(current, 0x24);
                if !storage.is_null() { free(storage); }
                let object = pointer(current, 0x0C);
                if !object.is_null() { vcall!(object, 8 => fn() -> u32); }
                let object = pointer(current, 8);
                if !object.is_null() { vcall!(object, 8 => fn() -> u32); }
                free(current);
                header = pointer(this, 0);
            }
            let first_header = pointer(this, 0);
            put_pointer(first_header, 0, first_header);
            let final_header = pointer(this, 0);
            put_pointer(final_header, 4, final_header);
            final_header
        }
    }
}

port! {
    /// Releases each node's interface before freeing it. The empty path keeps the incoming EDX.
    #[port(address = 0x0058_4210, owner = "shared", name = "ClearInterfaceList")]
    pub unsafe extern "fastcall" fn fc4p_00584210(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe {
            let mut header = pointer(this, 0);
            let mut node = pointer(header, 0);
            let mut edx = entry_edx;
            while node != header {
                let current = node;
                let object = pointer(current, 0x0C);
                node = pointer(node, 0);
                let mut registers = [header as u32, 0, edx];
                if !object.is_null() {
                    registers = virtual_registers(object, 8, pointer(object, 0) as u32, edx);
                }
                let args = [current as u32];
                let mut call = crate::abi::RawCall::new(crate::abi::Conv::Cdecl, crate::abi::target(0x0090_CF63), registers[1], &args);
                call.eax = registers[0]; call.edx = registers[2];
                crate::abi::call_raw(&mut call);
                edx = call.edx;
                header = pointer(this, 0);
            }
            let first_header = pointer(this, 0);
            put_pointer(first_header, 0, first_header);
            let final_header = pointer(this, 0);
            put_pointer(final_header, 4, final_header);
            (u64::from(edx) << 32) | u64::from(first_header as u32)
        }
    }
}

port! {
    /// Allocates a pool chunk, links the old chunk and chains its fixed-size entries into a free list.
    /// EDX is the final chain cursor, which can differ from the terminal link's address on wrapped sizes.
    #[port(address = 0x005A_61B0, owner = "shared", name = "GrowFixedSizePool")]
    pub unsafe extern "thiscall" fn fc4p_005a61b0(this: *mut u8) -> u64 {
        unsafe {
            let bytes = read_unaligned(this.add(4).cast::<u32>());
            let chunk = allocate_pool_block(bytes.wrapping_add(8));
            put_pointer(chunk, 0, pointer(this, 8));
            let mut stride = read_unaligned(this.cast::<u32>());
            let first = chunk.wrapping_add(4);
            put_pointer(this, 8, chunk);
            let mut quotient = read_unaligned(this.add(4).cast::<u32>());
            core::arch::asm!("xor edx, edx", "div ecx", inout("eax") quotient, in("ecx") stride, out("edx") _);
            let last = first.wrapping_add(quotient.wrapping_sub(1).wrapping_mul(stride) as usize);
            let mut cursor = first;
            while (cursor as u32) < last as u32 {
                put_pointer(cursor, 0, cursor.wrapping_add(stride as usize));
                stride = read_unaligned(this.cast::<u32>());
                cursor = cursor.wrapping_add(stride as usize);
            }
            put_pointer(last, 0, core::ptr::null_mut());
            put_pointer(this, 12, first);
            (u64::from(cursor as u32) << 32) | u64::from(last as u32)
        }
    }
}

port! {
    /// Takes the first free slot, allocating a chunk only when the free list is empty.
    #[port(address = 0x005A_6290, owner = "shared", name = "AllocateFixedSizePoolSlot", naked)]
    pub unsafe extern "C" fn fc4p_005a6290() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp", "call {body}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret",
            body = sym allocate_pool_slot);
    }
}

unsafe extern "fastcall" fn allocate_pool_slot(this: *mut u8, registers: *mut [u32; 3]) {
    unsafe {
        let mut edx = (*registers)[2];
        if pointer(this, 12).is_null() {
            edx = (grow_pool(this) >> 32) as u32;
        }
        let slot = pointer(this, 12);
        let next = pointer(slot, 0);
        put_pointer(this, 12, next);
        registers.write([slot as u32, next as u32, edx]);
    }
}

port! {
    /// Adds sinusoidal displacement from seven-float wave records. All phases use the initial point.
    /// The y and z contributions round to float before addition; x retains its register exponent range.
    #[port(address = 0x005A_C070, owner = "shared", name = "ApplySinusoidalDisplacement")]
    pub unsafe extern "C" fn fc4p_005ac070(time: f32, waves: *mut u8, point: *mut f32) {
        unsafe {
            let original = [read_unaligned(point), read_unaligned(point.add(1)), read_unaligned(point.add(2))];
            let mut record = pointer(waves, 0);
            if record == pointer(waves, 4) { return; }
            loop {
                let get = |offset| read_unaligned(record.add(offset).cast::<f32>());
                let z = multiply_float(F80::from_f32(original[2]), get(12));
                let x = multiply_float(F80::from_f32(original[0]), get(4));
                let phase = add_popped(z, x);
                let phase = add_popped(phase, multiply_float(F80::from_f32(time), get(0)));
                let phase = add_popped(phase, multiply_float(F80::from_f32(original[1]), get(8)));
                let wave = phase.sin();
                let dx = multiply_float(wave, get(16));
                let dy = multiply_float(wave, get(20)).to_f32();
                let dz = multiply_float(wave, get(24)).to_f32();
                write_unaligned(point, add_float(dx, read_unaligned(point)).to_f32());
                write_unaligned(point.add(1), add_float(F80::from_f32(dy), read_unaligned(point.add(1))).to_f32());
                write_unaligned(point.add(2), add_float(F80::from_f32(dz), read_unaligned(point.add(2))).to_f32());
                record = record.wrapping_add(28);
                if record == pointer(waves, 4) { break; }
            }
        }
    }
}

// All field accesses use the game's storage, including fields a callback may change.
unsafe fn pointer(p: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

unsafe fn put_pointer(p: *mut u8, offset: usize, value: *mut u8) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

layout! {
    /// Iterators of a deque with sixteen eight-byte entries per block.
    struct EntryDeque {
        0x00 => first / set_first: *mut u8,
        0x04 => first_block / set_first_block: *mut u8,
        0x08 => first_limit / set_first_limit: *mut u8,
        0x0C => first_map / set_first_map: *mut u8,
        0x10 => last / set_last: *mut u8,
        0x14 => last_block / set_last_block: *mut u8,
        0x18 => last_limit / set_last_limit: *mut u8,
        0x1C => last_map / set_last_map: *mut u8,
        0x20 => map / set_map: *mut u8,
        0x24 => map_capacity / set_map_capacity: u32,
    }
}

// The forwarding entries retain every register and the original stack arguments.
macro_rules! adjusted_entry {
    ($export:ident, $address:literal, $adjust:literal, $target:literal, $name:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!("sub ecx, {adjust}", "push {target}", "ret",
                    adjust = const $adjust, target = const $target);
                #[cfg(test)]
                core::arch::naked_asm!("sub ecx, {adjust}", "push {target}", "jmp {resolve}",
                    adjust = const $adjust, target = const $target, resolve = sym forward_test_target);
            }
        }
    };
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn forward_test_target() {
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push [esp + 12]",
        "call {resolve}", "mov [esp + 12], eax", "pop edx", "pop ecx", "pop eax", "ret",
        resolve = sym resolve_test_target);
}

#[cfg(test)]
extern "stdcall" fn resolve_test_target(address: u32) -> usize {
    crate::abi::target(address)
}

adjusted_entry!(
    fc4p_0055b3a0,
    0x0055_B3A0,
    12,
    0x0053_CCE0,
    "DestroyVehicleFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_00565580,
    0x0056_5580,
    40,
    0x004B_D280,
    "RetainDispatchFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_00565590,
    0x0056_5590,
    40,
    0x004B_D290,
    "ReleaseDispatchFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_0057db80,
    0x0057_DB80,
    16,
    0x0057_EB00,
    "RetainEffectFromSecondaryInterface"
);
adjusted_entry!(
    fc4p_0057db90,
    0x0057_DB90,
    16,
    0x0057_EB70,
    "ReleaseEffectFromSecondaryInterface"
);

port! {
    /// Allocates with tag 16; the caller's single argument is removed on return.
    /// The two hidden register parameters preserve ECX and EDX at the allocation call.
    #[port(address = 0x0055_B7C0, owner = "shared", name = "AllocateWithTag16")]
    pub unsafe extern "fastcall" fn fc4p_0055b7c0(entry_ecx: u32, entry_edx: u32, size: u32) -> u64 {
        let args = [size, 16];
        let mut call = crate::abi::RawCall::new(crate::abi::Conv::Cdecl, crate::abi::target(0x0090_D91C), entry_ecx, &args);
        call.eax = size;
        call.edx = entry_edx;
        unsafe { crate::abi::call_raw(&mut call); }
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

port! {
    /// Returns a vehicle allocation to the size-specific pool or the general allocator.
    #[port(address = 0x0055_CA60, owner = "shared", name = "FreeVehicleBySize")]
    pub unsafe extern "C" fn fc4p_0055ca60(storage: *mut u8, size: u32) -> u32 {
        unsafe {
            if storage.is_null() { 0 }
            else if size == 0x2B8 { free_vehicle_pool(vehicle_pool(), storage) }
            else { free_vehicle(storage) }
        }
    }
}

port! {
    /// Releases the first entry and advances the front iterator, freeing a completed block.
    #[port(address = 0x0055_FAF0, owner = "shared", name = "PopFrontEntryDeque")]
    pub unsafe extern "thiscall" fn fc4p_0055faf0(this: *mut u8) -> *mut u8 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let at_end = deque.first() == deque.first_limit().wrapping_sub(8);
            let item = pointer(deque.first(), 0);
            if !item.is_null() { vcall!(item, 8 => fn() -> u32); }
            if !at_end {
                let next = deque.first().wrapping_add(8);
                deque.set_first(next);
                return next;
            }
            if !deque.first_block().is_null() { free(deque.first_block()); }
            let next_map = deque.first_map().wrapping_add(4);
            deque.set_first_map(next_map);
            let block = pointer(next_map, 0);
            deque.set_first_block(block);
            deque.set_first_limit(block.wrapping_add(0x80));
            let next = deque.first_block();
            deque.set_first(next);
            next
        }
    }
}

port! {
    /// Adds a new back block and constructs the final entry in the old block, retaining its interface.
    #[port(address = 0x0056_0250, owner = "shared", name = "AppendEntryDequeBlock")]
    pub unsafe extern "thiscall" fn fc4p_00560250(this: *mut u8, value: *const u8) -> u32 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let item = pointer(value.cast_mut(), 0);
            if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
            let payload = read_unaligned(value.add(4).cast::<u32>());
            let used = (deque.last_map() as u32).wrapping_sub(deque.map() as u32) as i32 >> 2;
            if deque.map_capacity().wrapping_sub(used as u32) < 2 { grow_deque_map(this, 1, 0); }
            let block = allocate(0x80);
            put_pointer(deque.last_map(), 4, block);
            let destination = deque.last();
            if !destination.is_null() {
                put_pointer(destination, 0, item);
                if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
                write_unaligned(destination.add(4).cast::<u32>(), payload);
            }
            let next_map = deque.last_map().wrapping_add(4);
            deque.set_last_map(next_map);
            let block = pointer(next_map, 0);
            deque.set_last_block(block);
            deque.set_last_limit(block.wrapping_add(0x80));
            let next = deque.last_block();
            deque.set_last(next);
            if !item.is_null() { vcall!(item, 8 => fn() -> u32) } else { next as u32 }
        }
    }
}

port! {
    /// Appends an interface and payload, delegating the last slot of a block to the growth helper.
    #[port(address = 0x0056_14B0, owner = "shared", name = "AppendEntryDeque")]
    pub unsafe extern "thiscall" fn fc4p_005614b0(this: *mut u8, value: *const u8) -> u32 {
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            let limit = deque.last_limit().wrapping_sub(8);
            let destination = deque.last();
            if destination == limit { return append_deque_block(this, value); }
            let mut result = limit as u32;
            if !destination.is_null() {
                let item = pointer(value.cast_mut(), 0);
                put_pointer(destination, 0, item);
                if !item.is_null() { vcall!(item, 4 => fn() -> u32); }
                result = read_unaligned(value.add(4).cast::<u32>());
                write_unaligned(destination.add(4).cast::<u32>(), result);
            }
            deque.set_last(deque.last().wrapping_add(8));
            result
        }
    }
}

port! {
    /// Returns the address of the embedded instance collection.
    #[port(address = 0x0057_7450, owner = "shared", name = "GetInstances", naked)]
    pub unsafe extern "C" fn fc4p_00577450() {
        core::arch::naked_asm!("lea eax, [ecx + 12]", "ret");
    }
}

port! {
    /// Returns the fixed class identifier while retaining the other registers.
    #[port(address = 0x0057_F300, owner = "shared", name = "GetEffectClassId", naked)]
    pub unsafe extern "C" fn fc4p_0057f300() {
        core::arch::naked_asm!("mov eax, 0xc4c90997", "ret");
    }
}

port! {
    /// Reads the flag byte, retaining the caller's upper EAX bytes.
    #[port(address = 0x0058_0B50, owner = "shared", name = "GetFlagByte", naked)]
    pub unsafe extern "C" fn fc4p_00580b50() {
        core::arch::naked_asm!("mov al, [ecx + 0x1c]", "ret");
    }
}

port! {
    /// Appends one retained interface pointer, growing the vector when full.
    #[port(address = 0x0059_D7E0, owner = "shared", name = "AppendInterfaceVector")]
    pub unsafe extern "thiscall" fn fc4p_0059d7e0(this: *mut u8, value: *const *mut u8) -> u32 {
        unsafe {
            let end = pointer(this, 4);
            if end == pointer(this, 8) {
                let mut guard = 0u8;
                return insert_vector(this, end, value.cast(), &mut guard, 1, 1);
            }
            let mut result = end as u32;
            if !end.is_null() {
                let item = read_unaligned(value);
                put_pointer(end, 0, item);
                if !item.is_null() { result = vcall!(item, 4 => fn() -> u32); }
            }
            put_pointer(this, 4, pointer(this, 4).wrapping_add(4));
            result
        }
    }
}

port! {
    /// Returns the city's zone-manager interface without retaining it.
    #[port(address = 0x005A_4DE0, owner = "shared", name = "GetZoneManager", naked)]
    pub unsafe extern "C" fn fc4p_005a4de0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x134]", "ret");
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_005504e0::PORT,
    fc4p_005527e0::PORT,
    fc4p_00553aa0::PORT,
    fc4p_00561910::PORT,
    fc4p_0055b3a0::PORT,
    fc4p_0055b7c0::PORT,
    fc4p_0055ca60::PORT,
    fc4p_0055faf0::PORT,
    fc4p_00560250::PORT,
    fc4p_005614b0::PORT,
    fc4p_00565580::PORT,
    fc4p_00565590::PORT,
    fc4p_00568ba0::PORT,
    fc4p_00568c10::PORT,
    fc4p_00570c50::PORT,
    fc4p_00577450::PORT,
    fc4p_0057db80::PORT,
    fc4p_0057db90::PORT,
    fc4p_0057f300::PORT,
    fc4p_00580b50::PORT,
    fc4p_00584210::PORT,
    fc4p_00586ef0::PORT,
    fc4p_0059d7e0::PORT,
    fc4p_005a4de0::PORT,
    fc4p_005a61b0::PORT,
    fc4p_005a6290::PORT,
    fc4p_005ac070::PORT,
];

#[cfg(test)]
mod explicit_route_tests {
    use super::*;
    use crate::abi::{Conv, RawCall};
    use crate::testing::{fake, fake_data, Fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Manager(usize),
        Cell(usize, u32, u32),
        Free(usize),
        Next(usize, u32, u32, u32, u32),
        Accept(usize, u32, u32, u32),
        Assign(usize, usize),
    }
    #[derive(Default)]
    struct State {
        events: Vec<Event>,
        manager: usize,
        list: usize,
        next: usize,
        accepted: Vec<u8>,
        terminal: usize,
        rejected: usize,
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn log(event: Event) {
        STATE.with(|s| s.borrow_mut().events.push(event));
    }
    fn events() -> Vec<Event> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
    }
    extern "thiscall" fn manager(this: *mut u8) -> *mut u8 {
        log(Event::Manager(this as usize));
        STATE.with(|s| s.borrow().manager as *mut u8)
    }
    extern "thiscall" fn cell(this: *mut u8, x: u32, z: u32) -> u64 {
        log(Event::Cell(this as usize, x, z));
        (0x1111_2222u64 << 32) | STATE.with(|s| s.borrow().list as u64)
    }
    extern "thiscall" fn next(this: *mut u8, previous: u32, word: u32, one: u32, selector: u32) -> u64 {
        log(Event::Next(this as usize, previous, word, one, selector));
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(unsafe { *(s.terminal as *const u8) }, 1);
            (0x3333_4444u64 << 32) | s.next as u64
        })
    }
    extern "thiscall" fn accept(this: *mut u8, selector: u32, entry: u32, exit: u32) -> u64 {
        log(Event::Accept(this as usize, selector, entry, exit));
        let accepted = STATE.with(|s| {
            let s = s.borrow();
            this as usize != s.rejected && s.accepted.contains(&(exit as u8))
        });
        (0x5555_6666u64 << 32) | 0xAABB_CC00 | u64::from(accepted)
    }
    extern "thiscall" fn assign(this: *mut u8, network: *mut u8) -> u64 {
        log(Event::Assign(this as usize, network as usize));
        unsafe {
            put_pointer(this, 0, network);
        }
        0x7777_8888_9999_AAAA
    }
    extern "C" fn delete(block: *mut u8) {
        log(Event::Free(block as usize));
    }
    #[unsafe(naked)]
    unsafe extern "C" fn seeded_entry() {
        core::arch::naked_asm!("mov dword ptr [esp - 12], 0x55667788", "jmp {port}", port = sym fc4p_005527e0);
    }
    struct Fixture {
        _city_table: Box<[usize; 41]>,
        _manager_table: Box<[usize; 16]>,
        _network_table: Box<[usize; 54]>,
        _city: Box<[usize; 1]>,
        _global: Box<usize>,
        manager: Box<[usize; 1]>,
        network: Box<[usize; 1]>,
        _guards: Vec<Fake>,
        object: Object,
    }
    impl Fixture {
        fn new() -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut ct = Box::new([0usize; 41]);
            ct[40] = manager as *const () as usize;
            let mut mt = Box::new([0usize; 16]);
            mt[14] = cell as *const () as usize;
            mt[15] = next as *const () as usize;
            let mut nt = Box::new([0usize; 54]);
            nt[53] = accept as *const () as usize;
            let city = Box::new([ct.as_ptr() as usize]);
            let mut global = Box::new(city.as_ptr() as usize);
            let manager = Box::new([mt.as_ptr() as usize]);
            let network = Box::new([nt.as_ptr() as usize]);
            STATE.with(|s| s.borrow_mut().manager = manager.as_ptr() as usize);
            let guards = vec![
                fake_data(0x00B4_3CEC, (&mut *global as *mut usize).cast()),
                fake(0x0054_47B0, assign as *const () as usize),
                fake(0x0090_CF63, delete as *const () as usize),
            ];
            Self {
                _city_table: ct,
                _manager_table: mt,
                _network_table: nt,
                _city: city,
                _global: global,
                manager,
                network,
                _guards: guards,
                object: Object::new(0x250),
            }
        }
        fn route(&mut self, bytes: &mut [u8], end: usize) {
            unsafe {
                let p = self.object.ptr();
                put_pointer(p, 0x22C, bytes.as_mut_ptr());
                put_pointer(p, 0x230, bytes.as_mut_ptr());
                put_pointer(p, 0x234, bytes.as_mut_ptr().add(bytes.len()));
                put_pointer(p, 0x23C, bytes.as_mut_ptr().add(end));
            }
        }
        fn run(&mut self, args: &mut [u32; 2]) -> (u32, u32) {
            let mut call = RawCall::new(
                Conv::Thiscall,
                seeded_entry as *const () as usize,
                self.object.ptr() as u32,
                args,
            );
            call.args = args.as_mut_ptr();
            call.edx = 0xDEAD_BEEF;
            unsafe {
                super::tests::call_with_stack(&mut call);
            }
            (call.eax, call.edx)
        }
    }

    #[test]
    fn empty_route_and_exhausted_lists_return_zero_without_changing_output() {
        let mut f = Fixture::new();
        let mut bytes = [4u8, 5, 9, 6, 7, 0, 0, 0, 0];
        let mut previous = [0x1234u32, 0xA3EE_0102];
        let mut output = [0xCCCC_CCCCu32; 3];
        let mut args = [previous.as_mut_ptr() as u32, output.as_mut_ptr() as u32];
        f.route(&mut bytes, 0);
        assert_eq!(f.run(&mut args), (0, bytes.as_mut_ptr() as u32));
        assert_eq!(events(), [Event::Manager(f._city.as_ptr() as usize)]);
        let saved = args;
        f.route(&mut bytes, 6);
        assert_eq!(f.run(&mut args), (0, 0x1111_2222));
        assert_eq!(args, [(saved[0] & !0xFF) | 3, saved[1]]);
        assert_eq!(output, [0xCCCC_CCCC; 3]);
        assert_eq!(
            events(),
            [
                Event::Manager(f._city.as_ptr() as usize),
                Event::Cell(f.manager.as_ptr() as usize, 4, 5)
            ]
        );
    }

    #[test]
    fn explicit_consumption_diagonal_duplicate_and_direction_order_match_route_cases() {
        // A diagonal chooses its z direction first; equal cells choose exit two.
        for (next_cell, exit) in [
            ([3u8, 4, 0], 1u32),
            ([5, 6, 0], 3),
            ([3, 5, 0], 0),
            ([4, 5, 0], 2),
            ([5, 5, 0], 2),
        ] {
            let mut f = Fixture::new();
            let np = f.network.as_mut_ptr() as usize;
            let mut tail = [0usize; 22];
            tail[0] = np;
            let mut rejected = [0usize; 1];
            rejected[0] = f._network_table.as_ptr() as usize;
            let rp = rejected.as_mut_ptr() as usize;
            let mut middle = [0usize; 22];
            middle[0] = rp;
            middle[21] = tail.as_mut_ptr() as usize;
            // A null network is skipped before the accepting node.
            let mut head = [0usize; 22];
            head[21] = middle.as_mut_ptr() as usize;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.list = head.as_mut_ptr() as usize;
                s.accepted = vec![exit as u8];
                s.rejected = rp;
            });
            let mut bytes = [4u8, 5, 9, next_cell[0], next_cell[1], next_cell[2], 0, 0, 0];
            f.route(&mut bytes, 6);
            let mut previous = [0x1234u32, 0xA3EE_0102];
            let mut output = [0xCCCC_CCCCu32; 3];
            let op = output.as_mut_ptr() as usize;
            let mut args = [previous.as_mut_ptr() as u32, op as u32];
            let before = args;
            assert_eq!(f.run(&mut args), (1, 0x7777_8888));
            assert_eq!(output, [np as u32, 0x0300_0000 | exit, 0xCCCC_CC00]);
            assert_eq!(args, [(before[0] & !0xFF) | exit, before[1]]);
            assert_eq!(unsafe { pointer(f.object.ptr(), 0x22C) }, unsafe {
                bytes.as_mut_ptr().add(3)
            });
            assert_eq!(
                events(),
                [
                    Event::Manager(f._city.as_ptr() as usize),
                    Event::Cell(f.manager.as_ptr() as usize, 4, 5),
                    Event::Accept(rp, 3, 0x5566_7700, (before[0] & !0xFF) | exit),
                    Event::Accept(np, 3, 0x5566_7700, (before[0] & !0xFF) | exit),
                    Event::Assign(op, np)
                ]
            );
        }
    }

    #[test]
    fn terminal_route_tries_four_exits_assigns_before_acceptance_and_keeps_argument_upper_bytes() {
        for accepted in [Some(3u8), Some(0), Some(1), Some(2), None] {
            let mut f = Fixture::new();
            let np = f.network.as_mut_ptr() as usize;
            let mut found = [np];
            let mut bytes = [4u8, 5, 9, 0, 0, 0];
            f.route(&mut bytes, 3);
            let mut previous = [0x1234u32, 0xA3EE_0103];
            let mut output = [0xCCCC_CCCCu32; 3];
            let op = output.as_mut_ptr() as usize;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.next = found.as_mut_ptr() as usize;
                s.accepted = accepted.into_iter().collect();
                s.terminal = op + 8;
            });
            let mut args = [previous.as_mut_ptr() as u32, op as u32];
            let before = args;
            let result = f.run(&mut args);
            let count = accepted.map_or(4, |exit| ((exit + 1) % 4) as usize + 1);
            let mut expected = vec![
                Event::Manager(f._city.as_ptr() as usize),
                Event::Cell(f.manager.as_ptr() as usize, 4, 5),
                Event::Next(f.manager.as_ptr() as usize, previous[0], previous[1], 1, 3),
                Event::Assign(op, np),
            ];
            for exit in [3u32, 0, 1, 2].into_iter().take(count) {
                expected.push(Event::Accept(np, 3, 0x5566_7701, (before[0] & !0xFF) | exit));
            }
            assert_eq!(events(), expected);
            assert_eq!(output[0], np as u32);
            assert_eq!(output[2], 0xCCCC_CC01);
            if let Some(exit) = accepted {
                assert_eq!(result, (0xAABB_CC01, u32::from(exit)));
                assert_eq!(output[1], 0x0300_0100 | u32::from(exit));
                assert_eq!(
                    args,
                    [
                        (before[0] & !0xFF) | u32::from(exit),
                        (before[1] & !0xFF) | (count as u32 - 1)
                    ]
                );
            } else {
                assert_eq!(result, (0xAABB_CC00, 0x5555_6666));
                assert_eq!(output[1], 0xCCCC_CCCC);
                assert_eq!(args, [(before[0] & !0xFF) | 3, (before[1] & !0xFF) | 4]);
            }
        }
    }

    #[test]
    fn terminal_lookup_failure_sets_only_the_terminal_byte_and_boundary_pop_frees_the_old_block() {
        let mut f = Fixture::new();
        let mut old = [0u8; 126];
        old[123..].copy_from_slice(&[4, 5, 9]);
        let mut new = [0u8; 126];
        let mut map = [old.as_mut_ptr() as usize, new.as_mut_ptr() as usize];
        f.route(&mut old, 0);
        unsafe {
            put_pointer(f.object.ptr(), 0x22C, old.as_mut_ptr().add(123));
            put_pointer(f.object.ptr(), 0x23C, new.as_mut_ptr());
            put_pointer(f.object.ptr(), 0x238, map.as_mut_ptr().cast());
        }
        let mut previous = [0x1234u32, 0xA3EE_0103];
        let mut output = [0xCCCC_CCCCu32; 3];
        let op = output.as_mut_ptr() as usize;
        STATE.with(|s| s.borrow_mut().terminal = op + 8);
        let mut args = [previous.as_mut_ptr() as u32, op as u32];
        let before = args;
        assert_eq!(f.run(&mut args), (0, 0x3333_4444));
        assert_eq!(args, before);
        assert_eq!(output, [0xCCCC_CCCC, 0xCCCC_CCCC, 0xCCCC_CC01]);
        assert_eq!(
            events(),
            [
                Event::Manager(f._city.as_ptr() as usize),
                Event::Cell(f.manager.as_ptr() as usize, 4, 5),
                Event::Free(old.as_mut_ptr() as usize),
                Event::Next(f.manager.as_ptr() as usize, previous[0], previous[1], 1, 3)
            ]
        );
        unsafe {
            assert_eq!(pointer(f.object.ptr(), 0x22C), new.as_mut_ptr());
            assert_eq!(pointer(f.object.ptr(), 0x230), new.as_mut_ptr());
            assert_eq!(pointer(f.object.ptr(), 0x234), new.as_mut_ptr().add(126));
            assert_eq!(pointer(f.object.ptr(), 0x238), map.as_mut_ptr().add(1).cast());
        }
    }
}

#[cfg(test)]
mod rebuild_tests {
    use super::*;
    use crate::testing::{fake, Fake, Object};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Ready(usize),
        Flag(usize, u32),
        Refresh(usize),
        Stop(usize, u32),
        Release(usize),
        Pop(usize),
        Position(usize),
        Begin(usize),
        Query(usize, [u32; 4], [u32; 3]),
        Service(usize),
        Path(usize, u32, u32),
        Next(usize),
        Start(usize, u32, u32, u32),
        Arrived(usize),
        Arrival(usize),
    }
    struct State {
        events: Vec<Event>,
        ready: u32,
        flags: u32,
        queries: VecDeque<Option<u32>>,
        arrived: u32,
        cleared: usize,
        remove_on_refresh: bool,
        remove_on_next: bool,
        identifier: usize,
        replace_identifier: Option<u32>,
    }
    impl Default for State {
        fn default() -> Self {
            Self {
                events: vec![],
                ready: 1,
                flags: 0x1234_5600,
                queries: VecDeque::new(),
                arrived: 0x3456_7800,
                cleared: 0,
                remove_on_refresh: false,
                remove_on_next: false,
                identifier: 0,
                replace_identifier: None,
            }
        }
    }
    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }
    fn events() -> Vec<Event> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().events))
    }
    fn log(event: Event) {
        STATE.with(|s| s.borrow_mut().events.push(event));
    }
    extern "thiscall" fn ready(this: *mut u8) -> u32 {
        log(Event::Ready(this as usize));
        STATE.with(|s| s.borrow().ready)
    }
    extern "thiscall" fn flag(this: *mut u8, mask: u32) -> u32 {
        log(Event::Flag(this as usize, mask));
        STATE.with(|s| s.borrow().flags)
    }
    extern "thiscall" fn refresh(this: *mut u8) {
        log(Event::Refresh(this as usize));
        if STATE.with(|s| s.borrow().remove_on_refresh) {
            unsafe {
                put_pointer(this, 0x214, pointer(this, 0x204));
            }
        }
    }
    extern "thiscall" fn stop(this: *mut u8, value: u32) -> u32 {
        log(Event::Stop(this as usize, value));
        0xAABB_CC55
    }
    extern "thiscall" fn release(this: *mut u8) {
        log(Event::Release(this as usize));
        STATE.with(|s| {
            let p = s.borrow().cleared;
            assert_ne!(p, 0);
            assert_eq!(unsafe { *(p as *const u32) }, 0);
        });
    }
    extern "thiscall" fn pop(this: *mut u8) -> u64 {
        log(Event::Pop(this as usize));
        unsafe {
            put_pointer(this, 0, pointer(this, 0).add(12));
        }
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn position(this: *mut u8) -> *mut u8 {
        log(Event::Position(this as usize));
        unsafe { this.add(0x40) }
    }
    extern "thiscall" fn begin(this: *mut u8, output: *mut u32) -> *mut u32 {
        log(Event::Begin(this as usize));
        unsafe {
            write_unaligned(output.cast::<[u32; 4]>(), read_unaligned(this.cast::<[u32; 4]>()));
        }
        output
    }
    extern "thiscall" fn query(
        this: *mut u8,
        a: u32,
        b: u32,
        c: u32,
        d: u32,
        point: *mut u32,
        id: *mut u32,
        distance: *mut u32,
    ) -> u32 {
        log(Event::Query(this as usize, [a, b, c, d], unsafe {
            point.cast::<[u32; 3]>().read()
        }));
        let result = STATE.with(|s| s.borrow_mut().queries.pop_front().unwrap());
        if let Some(value) = result {
            STATE.with(|s| s.borrow_mut().identifier = id as usize);
            unsafe {
                id.write(value);
                distance.write(0xF0F0_F0F0);
            }
            0xBCDE_1201
        } else {
            0xBCDE_1200
        }
    }
    extern "thiscall" fn service(this: *mut u8) -> *mut u8 {
        log(Event::Service(this as usize));
        STATE.with(|s| {
            let s = s.borrow();
            if let Some(value) = s.replace_identifier {
                unsafe {
                    (s.identifier as *mut u32).write(value);
                }
            }
        });
        unsafe { pointer(this, 4) }
    }
    extern "thiscall" fn path(this: *mut u8, id: u32, zero: u32) -> *mut u8 {
        log(Event::Path(this as usize, id, zero));
        unsafe { pointer(this, 4) }
    }
    extern "thiscall" fn next(this: *mut u32) {
        log(Event::Next(unsafe { this.read() } as usize));
        unsafe {
            this.write(this.read().wrapping_add(12));
        }
        if STATE.with(|s| s.borrow().remove_on_next) {
            let owner = STATE.with(|s| s.borrow().cleared);
            // The test supplies the first iterator word as the callback's mutation target.
            unsafe {
                *(owner as *mut u32) = this.read();
            }
        }
    }
    extern "thiscall" fn start(this: *mut u8, object: u32, id: u32, one: u32) -> u32 {
        log(Event::Start(this as usize, object, id, one));
        0x2345_67FE
    }
    extern "thiscall" fn arrived(this: *mut u8) -> u32 {
        log(Event::Arrived(this as usize));
        STATE.with(|s| s.borrow().arrived)
    }
    extern "thiscall" fn arrival(this: *mut u8) -> u32 {
        log(Event::Arrival(this as usize));
        0x4567_89AB
    }
    struct Fixture {
        _table: Box<[usize; 175]>,
        _guards: Vec<Fake>,
        object: Object,
    }
    impl Fixture {
        fn new() -> Self {
            STATE.with(|s| *s.borrow_mut() = State::default());
            let mut table = Box::new([0usize; 175]);
            for (slot, function) in [
                (0x2B8, ready as *const () as usize),
                (0x7C, flag as *const () as usize),
                (0x298, refresh as *const () as usize),
                (0x34, stop as *const () as usize),
                (0x230, position as *const () as usize),
                (0x2A4, query as *const () as usize),
                (0x20C, start as *const () as usize),
                (0x160, arrived as *const () as usize),
                (0x13C, arrival as *const () as usize),
            ] {
                table[slot / 4] = function;
            }
            let mut object = Object::new(0x230);
            unsafe {
                put_pointer(object.ptr(), 0, table.as_mut_ptr().cast());
                put_pointer(object.ptr(), 0x1EC, 0x1234usize as *mut u8);
                write_unaligned(object.ptr().add(0x40).cast::<[u32; 3]>(), [1, 2, 3]);
                write_unaligned(object.ptr().add(0x1E8), 3u8);
                write_unaligned(object.ptr().add(0x1E9), 0xABu8);
            }
            let guards = [
                (0x0055_3720, pop as *const () as usize),
                (0x006C_5FE0, begin as *const () as usize),
                (0x0050_4C40, next as *const () as usize),
            ]
            .map(|(a, f)| fake(a, f))
            .into();
            Self {
                _table: table,
                _guards: guards,
                object,
            }
        }
        fn nodes(&mut self, nodes: &mut [[u32; 3]]) -> *mut u8 {
            let dp = unsafe { self.object.ptr().add(0x204) };
            unsafe {
                write_unaligned(
                    dp.cast::<[u32; 4]>(),
                    [nodes.as_mut_ptr() as u32, 0x1111, 0x2222, 0x3333],
                );
                put_pointer(dp, 16, nodes.as_mut_ptr().add(nodes.len()).cast());
            }
            dp
        }
        fn run(&mut self) -> u32 {
            unsafe { fc4p_00553aa0(self.object.ptr(), 0x89AB_CDEF) }
        }
    }

    #[test]
    fn readiness_current_interface_and_flags_gate_all_other_work() {
        for case in 0..3 {
            let mut f = Fixture::new();
            let p = f.object.ptr();
            match case {
                0 => STATE.with(|s| s.borrow_mut().ready = 0),
                1 => unsafe {
                    put_pointer(p, 0x1EC, core::ptr::null_mut());
                },
                _ => STATE.with(|s| s.borrow_mut().flags = 0xABCD_EF01),
            }
            let before = unsafe { std::slice::from_raw_parts(p, 0x230) }.to_vec();
            assert_eq!(f.run(), if case == 2 { 0xABCD_EF00 } else { 0 });
            assert_eq!(unsafe { std::slice::from_raw_parts(p, 0x230) }, before);
            let mut expected = vec![Event::Ready(p as usize)];
            if case == 2 {
                expected.push(Event::Flag(p as usize, 0x2000));
            }
            assert_eq!(events(), expected);
        }
    }

    #[test]
    fn empty_terminal_and_consumed_queues_stop_with_the_last_calls_upper_bytes() {
        for case in 0..4 {
            let mut f = Fixture::new();
            let p = f.object.ptr();
            let table = [0usize, 0, release as *const () as usize];
            let mut network = [table.as_ptr() as usize];
            let np = network.as_mut_ptr() as u32;
            let mut nodes = [[np, 7, u32::from(case == 1) | 0xAABB_CC00]];
            let dp = f.nodes(if case == 0 { &mut [] } else { &mut nodes });
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.cleared = nodes.as_mut_ptr() as usize;
                s.remove_on_refresh = case == 3;
            });
            let result = f.run();
            assert_eq!(result, if case == 1 { 0x5566_7700 } else { 0xAABB_CC00 });
            let mut expected = vec![
                Event::Ready(p as usize),
                Event::Flag(p as usize, 0x2000),
                Event::Refresh(p as usize),
            ];
            if case == 1 {
                expected.extend([
                    Event::Release(np as usize),
                    Event::Stop(p as usize, 0),
                    Event::Pop(dp as usize),
                ]);
            } else if case == 2 {
                expected.extend([
                    Event::Release(np as usize),
                    Event::Pop(dp as usize),
                    Event::Stop(p as usize, 0),
                ]);
            } else {
                expected.push(Event::Stop(p as usize, 0));
            }
            assert_eq!(events(), expected);
            assert_eq!(unsafe { *p.add(0x1E8) }, 3);
            if matches!(case, 1 | 2) {
                assert_eq!(nodes[0][0], 0);
            }
        }
    }

    #[test]
    fn rebuilds_identifiers_carries_positions_and_starts_the_new_front() {
        let mut f = Fixture::new();
        let p = f.object.ptr();
        let mut network_table = [0usize; 50];
        network_table[49] = service as *const () as usize;
        let mut service_table = [0usize; 4];
        service_table[3] = path as *const () as usize;
        let mut endpoint = [7u32, 8, 9];
        let mut geometry = [
            endpoint.as_mut_ptr() as usize,
            endpoint.as_mut_ptr().wrapping_add(3) as usize,
        ];
        let mut svc = [service_table.as_ptr() as usize, geometry.as_mut_ptr() as usize];
        let sp = svc.as_mut_ptr() as usize;
        let mut network = [network_table.as_ptr() as usize, sp];
        let np = network.as_mut_ptr() as usize;
        let mut null_service = [network_table.as_ptr() as usize, 0];
        let ns = null_service.as_mut_ptr() as usize;
        let mut nodes = [
            [0u32, 0x10, 0],
            [np as u32, 0x20, 0xABCD_EF00],
            [ns as u32, 0x30, 0],
            [np as u32, 0x40, 0],
        ];
        let dp = f.nodes(&mut nodes);
        STATE.with(|s| s.borrow_mut().queries = [Some(0xAAA1), Some(0xAAA2), None].into());
        assert_eq!(f.run(), 0x2345_6701);
        let a = nodes.as_mut_ptr() as usize + 12;
        assert_eq!(
            events(),
            [
                Event::Ready(p as usize),
                Event::Flag(p as usize, 0x2000),
                Event::Refresh(p as usize),
                Event::Pop(dp as usize),
                Event::Position(p as usize),
                Event::Begin(dp as usize),
                Event::Query(p as usize, [a as u32, 0x1111, 0x2222, 0x3333], [1, 2, 3]),
                Event::Service(np),
                Event::Path(sp, 0xAAA1, 0),
                Event::Next(a),
                Event::Query(p as usize, [(a + 12) as u32, 0x1111, 0x2222, 0x3333], [7, 8, 9]),
                Event::Service(ns),
                Event::Next(a + 12),
                Event::Query(p as usize, [(a + 24) as u32, 0x1111, 0x2222, 0x3333], [7, 8, 9]),
                Event::Next(a + 24),
                Event::Start(p as usize, np as u32, 0xAAA1, 1),
            ]
        );
        assert_eq!(
            nodes,
            [
                [0, 0x10, 0],
                [np as u32, 0xAAA1, 0xABCD_EF00],
                [ns as u32, 0xAAA2, 0],
                [np as u32, 0x40, 0]
            ]
        );
        assert_eq!(unsafe { read_unaligned(p.add(0x1E8).cast::<u16>()) }, 0xAB02);
    }

    #[test]
    fn existing_geometry_null_path_and_arrival_branches_preserve_the_result_byte() {
        for arrived_value in [0x1234_5600, 0x1234_5601] {
            let mut f = Fixture::new();
            let p = f.object.ptr();
            let mut endpoint = [10u32, 11, 12];
            let mut geometry = [
                endpoint.as_mut_ptr() as usize,
                endpoint.as_mut_ptr().wrapping_add(3) as usize,
            ];
            unsafe {
                put_pointer(p, 0x1F8, geometry.as_mut_ptr().cast());
                write_unaligned(p.add(0x1E8), 0);
            }
            let mut nt = [0usize; 50];
            nt[49] = service as *const () as usize;
            let st = [0usize, 0, 0, path as *const () as usize];
            let mut svc = [st.as_ptr() as usize, 0];
            let sp = svc.as_mut_ptr() as usize;
            let mut network = [nt.as_ptr() as usize, sp];
            let np = network.as_mut_ptr() as usize;
            let mut nodes = [[0u32, 1, 0], [np as u32, 2, 0x1234_5601]];
            let dp = f.nodes(&mut nodes);
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.queries = [Some(77)].into();
                s.arrived = arrived_value;
                s.replace_identifier = Some(99);
            });
            assert_eq!(
                f.run(),
                if arrived_value as u8 == 0 {
                    0xAABB_CC01
                } else {
                    0x4567_8901
                }
            );
            let a = nodes.as_mut_ptr() as usize + 12;
            let mut expected = vec![
                Event::Ready(p as usize),
                Event::Flag(p as usize, 0x2000),
                Event::Refresh(p as usize),
                Event::Pop(dp as usize),
                Event::Begin(dp as usize),
                Event::Query(p as usize, [a as u32, 0x1111, 0x2222, 0x3333], [10, 11, 12]),
                Event::Service(np),
                Event::Path(sp, 99, 0),
                Event::Next(a),
                Event::Start(p as usize, np as u32, 77, 1),
                Event::Arrived(p as usize),
            ];
            expected.push(if arrived_value as u8 == 0 {
                Event::Stop(p as usize, 0)
            } else {
                Event::Arrival(p as usize)
            });
            assert_eq!(events(), expected);
            assert_eq!(unsafe { *p.add(0x1E8) }, 0);
        }
    }

    #[test]
    fn callback_queue_changes_are_observed_and_failed_queries_leave_identifiers() {
        let mut f = Fixture::new();
        let p = f.object.ptr();
        let mut nodes = [[0u32, 1, 0], [0, 2, 0], [0, 3, 0]];
        let dp = f.nodes(&mut nodes);
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.queries = [None].into();
            s.remove_on_next = true;
            s.cleared = p as usize + 0x214;
        });
        assert_eq!(f.run(), 0x2345_6700);
        let calls = events();
        assert_eq!(calls.iter().filter(|e| matches!(e, Event::Query(..))).count(), 1);
        assert_eq!(nodes, [[0, 1, 0], [0, 2, 0], [0, 3, 0]]);
        assert_eq!(calls.last(), Some(&Event::Start(p as usize, 0, 2, 1)));
        assert_eq!(unsafe { pointer(dp, 16) }, unsafe { nodes.as_mut_ptr().add(2).cast() });
    }
}

#[cfg(test)]
mod path_tests {
    use super::*;
    use crate::abi::{Conv, RawCall};
    use crate::testing::{fake_data, Fake, GameFpu, Object};
    use std::cell::RefCell;

    thread_local! {
        static TARGETS: RefCell<Vec<(usize, [u32; 3])>> = const { RefCell::new(Vec::new()) };
        static TRANSLATE: RefCell<[f32; 3]> = const { RefCell::new([0.; 3]) };
    }
    extern "thiscall" fn transform(this: *mut u8, point: *mut f32) -> u64 {
        unsafe {
            TARGETS.with(|t| t.borrow_mut().push((this as usize, point.cast::<[u32; 3]>().read())));
            TRANSLATE.with(|t| {
                for (i, offset) in t.borrow().iter().enumerate() {
                    if *offset != 0.0 {
                        point
                            .add(i)
                            .write(add_float(F80::from_f32(point.add(i).read()), *offset).to_f32());
                    }
                }
            });
        }
        0xDEAD_BEEF_FACE_BAAD
    }
    struct Fixture {
        _fpu: GameFpu,
        _data: Box<[u32; 4]>,
        _table: Box<[usize; 158]>,
        _guards: Vec<Fake>,
        object: Object,
    }
    impl Fixture {
        fn new() -> Self {
            TARGETS.with(|t| assert!(t.borrow().is_empty()));
            TRANSLATE.with(|t| *t.borrow_mut() = [0.; 3]);
            let mut data = Box::new([0xCAFE_BABE, 1.0f32.to_bits(), 1e-5f32.to_bits(), 0]);
            let guards = [0x00B2_293C, 0x00B0_A07C, 0x00A9_BCD8, 0x00A8_1054]
                .into_iter()
                .enumerate()
                .map(|(i, a)| fake_data(a, unsafe { data.as_mut_ptr().add(i).cast() }))
                .collect();
            let mut table = Box::new([0usize; 158]);
            table[157] = transform as *const () as usize;
            let mut object = Object::new(0x204);
            unsafe {
                put_pointer(object.ptr(), 0, table.as_mut_ptr().cast());
            }
            Self {
                _fpu: GameFpu::new(),
                _data: data,
                _table: table,
                _guards: guards,
                object,
            }
        }
        fn run(&mut self, args: &mut [u32; 6]) -> (u32, u32) {
            let mut call = RawCall::new(
                Conv::Thiscall,
                fc4p_005504e0 as *const () as usize,
                self.object.ptr() as u32,
                args,
            );
            call.args = args.as_mut_ptr();
            call.eax = 0x1122_3344;
            call.edx = 0x5566_7788;
            unsafe {
                super::tests::call_with_stack(&mut call);
            }
            (call.eax, call.edx)
        }
    }
    fn targets() -> Vec<(usize, [u32; 3])> {
        TARGETS.with(|t| core::mem::take(&mut *t.borrow_mut()))
    }
    fn vector(points: &mut [[f32; 3]]) -> [u32; 3] {
        [
            points.as_mut_ptr() as u32,
            points.as_mut_ptr().wrapping_add(points.len()) as u32,
            points.as_mut_ptr().wrapping_add(points.len()) as u32,
        ]
    }

    #[test]
    fn missing_path_and_finished_index_leave_arguments_and_output_as_required() {
        let mut f = Fixture::new();
        let mut source = [1.25f32, -2.0, 3.5];
        let mut output = [99f32; 5];
        let mut overflow = -10f32;
        let mut args = [
            7.0f32.to_bits(),
            source.as_mut_ptr() as u32,
            unsafe { output.as_mut_ptr().add(1) } as u32,
            0,
            0,
            &mut overflow as *mut f32 as u32,
        ];
        let before = args;
        let result = f.run(&mut args);
        assert_eq!(result, (args[5] & !0xFF, 0xCAFE_BABE));
        assert_eq!(args, before);
        assert_eq!(output, [99.; 5]);
        assert_eq!(overflow.to_bits(), 0);
        let mut points = [[5f32, 6., 7.]];
        let mut path = vector(&mut points);
        unsafe {
            put_pointer(f.object.ptr(), 0x1F8, path.as_mut_ptr().cast());
        }
        for index in [0u32, 1, 0x7FFF_FFFF] {
            unsafe {
                write_unaligned(f.object.ptr().add(0x1FC).cast::<u32>(), index);
            }
            assert_eq!(f.run(&mut args), (args[5] & !0xFF, 0));
            assert_eq!(output, [99., 1.25, -2., 3.5, 99.]);
            assert_eq!(overflow, 7.);
            assert_eq!(args, before);
        }
        args[5] = 0;
        assert_eq!(f.run(&mut args), (0, 0));
        assert!(targets().is_empty());
    }

    #[test]
    fn distance_equality_negative_step_transform_and_default_negative_index() {
        let mut f = Fixture::new();
        let mut points = [[0f32; 3], [3., 4., 0.]];
        let mut path = vector(&mut points);
        let vp = f.object.ptr();
        unsafe {
            put_pointer(vp, 0x1F8, path.as_mut_ptr().cast());
        }
        for (distance, expected, eax) in [
            (2.5f32, [1.5, 2., 0.], 0xFACE_0001),
            (5., [3., 4., 0.], 0xFACE_4001),
            (-2.5, [-1.5, -2., 0.], 0xFACE_0001),
        ] {
            let mut start = [0f32; 3];
            let mut output = [99f32; 5];
            let mut overflow = 123f32;
            unsafe {
                write_unaligned(vp.add(0x1FC).cast::<u32>(), u32::MAX);
            }
            let mut args = [
                distance.to_bits(),
                start.as_mut_ptr() as u32,
                unsafe { output.as_mut_ptr().add(1) } as u32,
                0,
                0,
                &mut overflow as *mut f32 as u32,
            ];
            assert_eq!(f.run(&mut args), (eax, 0xDEAD_BEEF));
            assert_eq!(&output[1..4], &expected);
            assert_eq!((output[0], output[4]), (99., 99.));
            assert_eq!(overflow.to_bits(), 0);
            assert_eq!(args[3], 1);
            assert_eq!(args[4], 5.0f32.to_bits());
            assert_eq!(args[0], distance.to_bits());
            assert_eq!(unsafe { read_unaligned(vp.add(0x1FC).cast::<u32>()) }, 0);
            assert_eq!(targets(), [(vp as usize, [3.0f32.to_bits(), 4.0f32.to_bits(), 0])]);
        }
        TRANSLATE.with(|t| *t.borrow_mut() = [3., -4., 0.]);
        let mut start = [0f32; 3];
        let mut out = [0f32; 3];
        let mut index = 0u32;
        let mut args = [
            3.0f32.to_bits(),
            start.as_mut_ptr() as u32,
            out.as_mut_ptr() as u32,
            &mut index as *mut u32 as u32,
            path.as_mut_ptr() as u32,
            0,
        ];
        assert_eq!(f.run(&mut args), (0xFACE_0001, 0xDEAD_BEEF));
        assert_eq!(out, [3., 0., 0.]);
        assert_eq!(args[4], 6.0f32.to_bits());
        assert_eq!(targets(), [(vp as usize, [3.0f32.to_bits(), 4.0f32.to_bits(), 0])]);
    }

    #[test]
    fn crossing_segments_updates_remaining_argument_and_extrapolates_the_last_direction() {
        let mut f = Fixture::new();
        let mut points = [[0f32; 3], [3., 4., 0.], [3., 4., 2.]];
        let mut path = vector(&mut points);
        let mut start = [0f32; 3];
        let mut index = 0u32;
        let mut out = [99f32; 3];
        let mut overflow = -1f32;
        let mut args = [
            8.0f32.to_bits(),
            start.as_mut_ptr() as u32,
            out.as_mut_ptr() as u32,
            &mut index as *mut u32 as u32,
            path.as_mut_ptr() as u32,
            &mut overflow as *mut f32 as u32,
        ];
        let result = f.run(&mut args);
        assert_eq!(result, (args[5] & !0xFF, 2));
        assert_eq!(out, [3., 4., 3.]);
        assert_eq!(overflow, 1.);
        assert_eq!(index, 2);
        assert_eq!(args[0], 3.0f32.to_bits());
        assert_eq!(&args[3..5], &[2, 2.0f32.to_bits()]);
        assert_eq!(
            targets(),
            [
                (f.object.ptr() as usize, [3.0f32.to_bits(), 4.0f32.to_bits(), 0]),
                (
                    f.object.ptr() as usize,
                    [3.0f32.to_bits(), 4.0f32.to_bits(), 2.0f32.to_bits()]
                ),
            ]
        );
    }

    #[test]
    fn tiny_zero_and_unordered_segments_take_the_original_branches() {
        let mut f = Fixture::new();
        for (end, distance, expected, index, low_result) in [
            (0f32, 1f32, 0f32, 1, 0x4000),
            (0., 0., 0., 0, 0x4001),
            (1e-6, 5e-7, 5e-13, 0, 1),
            (f32::from_bits(0x7FC0_1234), 1., 0., 1, 0x4500),
        ] {
            let mut points = [[0f32; 3], [end, 0., 0.]];
            let mut path = vector(&mut points);
            let mut start = [0f32; 3];
            let mut at = 0u32;
            let mut out = [99f32; 3];
            let mut args = [
                distance.to_bits(),
                start.as_mut_ptr() as u32,
                out.as_mut_ptr() as u32,
                &mut at as *mut u32 as u32,
                path.as_mut_ptr() as u32,
                0,
            ];
            let (eax, edx) = f.run(&mut args);
            assert_eq!(eax & 0xFFFF, low_result);
            assert_eq!(edx, if index == 0 { 0xDEAD_BEEF } else { 1 });
            assert_eq!(at, index);
            assert_eq!(out.map(f32::to_bits), [expected.to_bits(), 0, 0]);
            assert_eq!(targets().len(), 1);
        }
    }

    #[test]
    fn wide_exponents_preserve_the_float_environment_and_an_existing_stack_value() {
        let mut f = Fixture::new();
        let mut points = [[0f32; 3], [f32::from_bits(0x7F00_0000), 0., 0.]];
        let mut path = vector(&mut points);
        let mut start = [0f32; 3];
        let mut out = [0f32; 3];
        let mut index = 0u32;
        let mut args = [
            0x7E80_0000,
            start.as_mut_ptr() as u32,
            out.as_mut_ptr() as u32,
            &mut index as *mut u32 as u32,
            path.as_mut_ptr() as u32,
            0,
        ];
        let sentinel = 123.25f64;
        let mut restored = 0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = super::tests::float_environment();
        let result = f.run(&mut args);
        let after = super::tests::float_environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(before, after);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        assert_eq!(out.map(f32::to_bits), [0x7E80_0000, 0, 0]);
        assert_eq!(args[4], 0x7F00_0000);
        assert_eq!(result, (0xFACE_3801, 0xDEAD_BEEF));
        assert_eq!(targets().len(), 1);
    }

    #[test]
    fn overlapping_point_copy_is_sequential_and_negative_single_point_indices_skip_the_loop() {
        let mut f = Fixture::new();
        let mut points = [[0f32; 3]];
        let mut path = vector(&mut points);
        let mut point = [1f32, 2., 3., 4., 99.];
        let mut index = u32::MAX;
        let mut args = [
            7.0f32.to_bits(),
            point.as_mut_ptr() as u32,
            unsafe { point.as_mut_ptr().add(1) } as u32,
            &mut index as *mut u32 as u32,
            path.as_mut_ptr() as u32,
            0,
        ];
        assert_eq!(f.run(&mut args), (0, 1.0f32.to_bits()));
        assert_eq!(point, [1., 1., 1., 1., 99.]);
        assert_eq!(index, 0);
        assert_eq!(args[3], 0);
        assert!(targets().is_empty());
    }
}

#[cfg(test)]
mod recent_tests {
    use super::*;
    use crate::testing::{fake, Fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Flag(usize, u32),
        Assign(usize),
        Back(usize),
        PopBack(usize),
        Append(usize, [u32; 2]),
        Release(usize),
        PopFront(usize),
    }
    thread_local! {
        static EVENTS: RefCell<Vec<Event>> = const { RefCell::new(Vec::new()) };
        static FLAG: RefCell<u32> = const { RefCell::new(0x1234_5600) };
        static NULL_TEMP: RefCell<bool> = const { RefCell::new(false) };
        static CLEARED: RefCell<Vec<usize>> = const { RefCell::new(Vec::new()) };
    }
    fn log(event: Event) {
        EVENTS.with(|e| e.borrow_mut().push(event));
    }
    fn events() -> Vec<Event> {
        EVENTS.with(|e| core::mem::take(&mut *e.borrow_mut()))
    }
    extern "thiscall" fn flag(this: *mut u8, mask: u32) -> u64 {
        log(Event::Flag(this as usize, mask));
        (0x1020_3040u64 << 32) | FLAG.with(|f| u64::from(*f.borrow()))
    }
    extern "thiscall" fn assign(this: *mut u8, source: *mut u8) {
        log(Event::Assign(source as usize));
        let object = if NULL_TEMP.with(|n| *n.borrow()) {
            core::ptr::null_mut()
        } else {
            unsafe { pointer(source, 0) }
        };
        unsafe {
            put_pointer(this, 0, object);
        }
    }
    extern "thiscall" fn back(this: *mut u8) -> *mut u8 {
        log(Event::Back(this as usize));
        unsafe { EntryDeque::from_ptr(this).last().sub(8) }
    }
    extern "thiscall" fn pop_back(this: *mut u8) {
        log(Event::PopBack(this as usize));
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            assert!(pointer(deque.last().sub(8), 0).is_null());
            deque.set_last(deque.last().sub(8));
        }
    }
    extern "thiscall" fn append(this: *mut u8, source: *const u32) {
        let entry = unsafe { [source.read(), source.add(1).read()] };
        log(Event::Append(this as usize, entry));
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            write_unaligned(deque.last().cast::<[u32; 2]>(), entry);
            deque.set_last(deque.last().add(8));
        }
    }
    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(Event::Release(this as usize));
        CLEARED.with(|c| {
            for address in c.borrow().iter() {
                assert_eq!(unsafe { *(*address as *const u32) }, 0);
            }
        });
        0x8765_4321_1122_3344
    }
    extern "thiscall" fn pop_front(this: *mut u8) {
        log(Event::PopFront(this as usize));
        unsafe {
            let deque = EntryDeque::from_ptr(this);
            assert!(pointer(deque.first(), 0).is_null());
            if deque.first() == deque.first_limit().sub(8) {
                deque.set_first_map(deque.first_map().add(4));
                let block = pointer(deque.first_map(), 0);
                deque.set_first_block(block);
                deque.set_first_limit(block.add(128));
                deque.set_first(block);
            } else {
                deque.set_first(deque.first().add(8));
            }
        }
    }
    fn fakes() -> Vec<Fake> {
        FLAG.with(|f| *f.borrow_mut() = 0x1234_5600);
        NULL_TEMP.with(|n| *n.borrow_mut() = false);
        CLEARED.with(|c| c.borrow_mut().clear());
        assert!(events().is_empty());
        [
            (0x0054_DB10, assign as *const () as usize),
            (0x0055_F900, back as *const () as usize),
            (0x0055_FA90, pop_back as *const () as usize),
            (0x0056_14B0, append as *const () as usize),
            (0x0055_FAF0, pop_front as *const () as usize),
        ]
        .map(|(address, function)| fake(address, function))
        .into()
    }
    fn vehicle(table: &[usize], object: *mut u8) -> Object {
        let mut result = Object::new(0x200);
        unsafe {
            put_pointer(result.ptr(), 0, table.as_ptr().cast_mut().cast());
            put_pointer(result.ptr(), 0x1EC, object);
            write_unaligned(result.ptr().add(0x1F0).cast::<u32>(), 0x5555_5555);
            write_unaligned(result.ptr().add(0x1F4).cast::<u32>(), 0xAABB_CCDD);
        }
        result
    }
    fn deque(memory: &mut [u32; 10], blocks: &mut [u32], map: &mut [usize; 2], first: usize, end: usize) -> *mut u8 {
        let p = memory.as_mut_ptr().cast();
        map[0] = blocks.as_mut_ptr() as usize;
        map[1] = blocks.as_mut_ptr().wrapping_add(32) as usize;
        unsafe {
            let d = EntryDeque::from_ptr(p);
            d.set_first(blocks.as_mut_ptr().add(first * 2).cast());
            d.set_first_block(blocks.as_mut_ptr().cast());
            d.set_first_limit(blocks.as_mut_ptr().add(32).cast());
            d.set_first_map(map.as_mut_ptr().cast());
            let back_block = end / 16;
            d.set_last(blocks.as_mut_ptr().add(end * 2).cast());
            d.set_last_block(blocks.as_mut_ptr().add(back_block * 32).cast());
            d.set_last_limit(blocks.as_mut_ptr().add((back_block + 1) * 32).cast());
            d.set_last_map(map.as_mut_ptr().add(back_block).cast());
            d.set_map(map.as_mut_ptr().cast());
            d.set_map_capacity(2);
        }
        p
    }

    #[test]
    fn early_exits_keep_both_return_registers_and_use_only_the_limits_low_byte() {
        let _fakes = fakes();
        let mut table = [0usize; 32];
        table[31] = flag as *const () as usize;
        let mut object = [0usize];
        let op = object.as_mut_ptr().cast();
        let mut vehicle = vehicle(&table, op);
        let vp = vehicle.ptr();
        for (status, current, limit, eax) in [
            (0xAABB_CC7E, op, 1, 0xAABB_CC7E),
            (0xAABB_CC00, core::ptr::null_mut(), 1, 0),
            (0xAABB_CC00, op, 0, op as u32),
            (0xAABB_CC00, op, 0x100, op as u32),
        ] {
            FLAG.with(|f| *f.borrow_mut() = status);
            unsafe {
                put_pointer(vp, 0x1EC, current);
            }
            let before = unsafe { std::slice::from_raw_parts(vp, 0x200) }.to_vec();
            assert_eq!(
                unsafe { fc4p_00561910(vp, 0xDEAD_BEEF, core::ptr::null_mut(), limit) },
                (0x1020_3040u64 << 32) | u64::from(eax)
            );
            assert_eq!(unsafe { std::slice::from_raw_parts(vp, 0x200) }, before);
            assert_eq!(events(), [Event::Flag(vp as usize, 0x2000)]);
        }
    }

    #[test]
    fn records_payload_replaces_duplicate_and_clears_before_release() {
        let _fakes = fakes();
        let mut table = [0usize; 32];
        table[31] = flag as *const () as usize;
        let release_table = [0usize, 0, release as *const () as usize];
        let mut object = [release_table.as_ptr() as usize];
        let op = object.as_mut_ptr().cast();
        let mut vehicle = vehicle(&table, op);
        let vp = vehicle.ptr();
        for duplicate in [false, true] {
            let mut blocks = [0xCCCC_CCCCu32; 64];
            let mut map = [0usize; 2];
            let mut memory = [0u32; 10];
            let dp = deque(&mut memory, &mut blocks, &mut map, 0, usize::from(duplicate));
            if duplicate {
                blocks[0] = op as u32;
            }
            assert_eq!(unsafe { fc4p_00561910(vp, 0, dp, 0xFFFF_0002) }, 0x8765_4321_1122_3344);
            let mut expected = vec![Event::Flag(vp as usize, 0x2000), Event::Assign(vp as usize + 0x1EC)];
            if duplicate {
                expected.extend([
                    Event::Back(dp as usize),
                    Event::Release(op as usize),
                    Event::PopBack(dp as usize),
                ]);
            }
            expected.extend([
                Event::Append(dp as usize, [op as u32, 0xAABB_CCDD]),
                Event::Release(op as usize),
            ]);
            assert_eq!(events(), expected);
            assert_eq!(&blocks[..2], &[op as u32, 0xAABB_CCDD]);
            assert!(blocks[2..].iter().all(|v| *v == 0xCCCC_CCCC));
            assert_eq!(unsafe { EntryDeque::from_ptr(dp).last() }, unsafe {
                blocks.as_mut_ptr().add(2).cast()
            });
        }
    }

    #[test]
    fn trims_across_blocks_and_releases_nonnull_entries_in_order() {
        let _fakes = fakes();
        let mut table = [0usize; 32];
        table[31] = flag as *const () as usize;
        let release_table = [0usize, 0, release as *const () as usize];
        let mut current = [release_table.as_ptr() as usize];
        let mut old = [release_table.as_ptr() as usize];
        let cp = current.as_mut_ptr().cast();
        let op = old.as_mut_ptr().cast::<u8>();
        let mut vehicle = vehicle(&table, cp);
        let vp = vehicle.ptr();
        let mut blocks = [0xCCCC_CCCCu32; 64];
        let mut map = [0usize; 2];
        let mut memory = [0u32; 10];
        let dp = deque(&mut memory, &mut blocks, &mut map, 14, 17);
        blocks[28] = op as u32;
        blocks[30] = 0;
        blocks[32] = op as u32;
        CLEARED.with(|c| *c.borrow_mut() = vec![unsafe { blocks.as_mut_ptr().add(28) } as usize]);
        assert_eq!(unsafe { fc4p_00561910(vp, 0, dp, 1) }, 0x8765_4321_1122_3344);
        assert_eq!(
            events(),
            [
                Event::Flag(vp as usize, 0x2000),
                Event::Assign(vp as usize + 0x1EC),
                Event::Back(dp as usize),
                Event::Append(dp as usize, [cp as u32, 0xAABB_CCDD]),
                Event::Release(op as usize),
                Event::PopFront(dp as usize),
                Event::PopFront(dp as usize),
                Event::Release(op as usize),
                Event::PopFront(dp as usize),
                Event::Release(cp as usize),
            ]
        );
        assert_eq!((blocks[28], blocks[30], blocks[32]), (0, 0, 0));
        assert_eq!(&blocks[34..36], &[cp as u32, 0xAABB_CCDD]);
        let d = unsafe { EntryDeque::from_ptr(dp) };
        assert_eq!(d.first(), unsafe { blocks.as_mut_ptr().add(34).cast() });
        assert_eq!(d.first_map(), unsafe { map.as_mut_ptr().add(1).cast() });
        assert_eq!(d.first_block(), unsafe { blocks.as_mut_ptr().add(32).cast() });
    }

    #[test]
    fn null_temporary_keeps_size_registers_with_and_without_trimming() {
        let _fakes = fakes();
        let mut table = [0usize; 32];
        table[31] = flag as *const () as usize;
        let mut current = [0usize];
        let mut vehicle = vehicle(&table, current.as_mut_ptr().cast());
        let vp = vehicle.ptr();
        NULL_TEMP.with(|n| *n.borrow_mut() = true);
        for end in [0, 3] {
            let mut blocks = [0u32; 64];
            let mut map = [0usize; 2];
            let mut memory = [0u32; 10];
            let dp = deque(&mut memory, &mut blocks, &mut map, 0, end);
            let result = unsafe { fc4p_00561910(vp, 0, dp, 1) };
            let d = unsafe { EntryDeque::from_ptr(dp) };
            let edx = if end == 0 { 1u32 } else { 13 };
            assert_eq!(result, (u64::from(edx) << 32) | u64::from(d.first() as u32));
            let calls = events();
            assert!(!calls.iter().any(|c| matches!(c, Event::Release(_))));
            assert_eq!(calls.iter().filter(|c| matches!(c, Event::PopFront(_))).count(), end);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{call_thiscall_with_eax, fake, fake_data, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Clone, Debug, PartialEq, Eq)]
    enum Call {
        Retain(usize),
        Release(usize),
        Free(usize),
        Allocate(u32),
        Tag(u32, u32),
        VehiclePool(usize, usize),
        VehicleFree(usize),
        Forward(usize, u32),
        GrowMap(usize, u32, u32),
        AppendBlock(usize, usize),
        Insert(usize, usize, usize, u8, u32, u32),
        Position(usize),
        Radius(usize),
        PoolAllocate(u32),
        PoolGrow(usize),
        Hash(usize, usize),
    }

    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static ALLOCATION: RefCell<usize> = const { RefCell::new(0) };
        // A call may change one word in the caller's object or source entry.
        static MUTATION: RefCell<Option<(Call, usize, u32)>> = const { RefCell::new(None) };
        static WATCH: RefCell<Option<(usize, usize)>> = const { RefCell::new(None) };
        static BUCKET: RefCell<u32> = const { RefCell::new(0) };
        static SNAPSHOTS: RefCell<Vec<Vec<u8>>> = const { RefCell::new(Vec::new()) };
    }

    fn log(call: Call) {
        WATCH.with(|w| {
            if let Some((p, n)) = *w.borrow() {
                let bytes = unsafe { std::slice::from_raw_parts(p as *const u8, n) }.to_vec();
                SNAPSHOTS.with(|s| s.borrow_mut().push(bytes));
            }
        });
        MUTATION.with(|m| {
            let mut m = m.borrow_mut();
            if let Some((at, address, value)) = m.as_ref() {
                if *at == call {
                    unsafe { write_unaligned(*address as *mut u32, *value) };
                    *m = None;
                }
            }
        });
        CALLS.with(|c| c.borrow_mut().push(call));
    }

    fn take_calls() -> Vec<Call> {
        CALLS.with(|c| std::mem::take(&mut *c.borrow_mut()))
    }

    extern "thiscall" fn retain(this: *mut u8) -> u64 {
        log(Call::Retain(this as usize));
        0x9876_5432_AABB_CCDD
    }
    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(Call::Release(this as usize));
        0x8765_4321_1122_3344
    }
    extern "C" fn delete(p: *mut u8) -> u64 {
        log(Call::Free(p as usize));
        0x7654_3210_1234_5678
    }
    extern "C" fn allocation(size: u32) -> *mut u8 {
        log(Call::Allocate(size));
        ALLOCATION.with(|p| *p.borrow() as *mut u8)
    }
    extern "C" fn tag(size: u32, tag: u32) -> u64 {
        log(Call::Tag(size, tag));
        0xABC0_9876_5432_1000
    }
    extern "thiscall" fn pooled(this: *mut u8, p: *mut u8) -> u32 {
        log(Call::VehiclePool(this as usize, p as usize));
        0x1234_ABCD
    }
    extern "C" fn vehicle_delete(p: *mut u8) -> u32 {
        log(Call::VehicleFree(p as usize));
        0xABCD_1234
    }
    extern "thiscall" fn forwarded(this: *mut u8) -> u64 {
        log(Call::Forward(this as usize, 0));
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn forwarded_flag(this: *mut u8, flag: u32) -> u64 {
        log(Call::Forward(this as usize, flag));
        0x1122_3344_5566_7788
    }
    extern "thiscall" fn map_grow(this: *mut u8, count: u32, front: u32) {
        log(Call::GrowMap(this as usize, count, front));
    }
    extern "thiscall" fn block_append(this: *mut u8, value: *const u8) -> u32 {
        log(Call::AppendBlock(this as usize, value as usize));
        0xFEDC_BA98
    }
    extern "thiscall" fn vector_insert(
        this: *mut u8,
        at: *mut u8,
        value: *const u8,
        guard: *mut u8,
        count: u32,
        capacity: u32,
    ) -> u32 {
        log(Call::Insert(
            this as usize,
            at as usize,
            value as usize,
            unsafe { *guard },
            count,
            capacity,
        ));
        0x1357_9BDF
    }

    fn table() -> [usize; 3] {
        [0, retain as *const () as usize, release as *const () as usize]
    }
    fn interface(table: &[usize]) -> [usize; 1] {
        [table.as_ptr() as usize]
    }
    unsafe fn bytes(p: *mut u8, n: usize) -> Vec<u8> {
        unsafe { std::slice::from_raw_parts(p, n) }.to_vec()
    }
    fn mutation(call: Call, p: *mut u8, value: u32) {
        MUTATION.with(|m| *m.borrow_mut() = Some((call, p as usize, value)));
    }

    #[test]
    fn adjusted_entries_forward_the_same_arguments_and_results() {
        let mut object = Object::new(64);
        for (entry, target, offset, flag) in [
            (fc4p_0055b3a0 as *const () as usize, 0x0053_CCE0, 12, Some(0xF123_4567)),
            (fc4p_00565580 as *const () as usize, 0x004B_D280, 40, None),
            (fc4p_00565590 as *const () as usize, 0x004B_D290, 40, None),
            (fc4p_0057db80 as *const () as usize, 0x0057_EB00, 16, None),
            (fc4p_0057db90 as *const () as usize, 0x0057_EB70, 16, None),
        ] {
            let f = if flag.is_some() {
                forwarded_flag as *const () as usize
            } else {
                forwarded as *const () as usize
            };
            let _fake = fake(target, f);
            let args: Vec<_> = flag.into_iter().collect();
            let result = unsafe { call_thiscall_with_eax(entry, object.ptr().add(offset), &args, 0xDEAD_BEEF) };
            assert_eq!(result, (0x5566_7788, 0x1122_3344));
            assert_eq!(take_calls(), [Call::Forward(object.ptr() as usize, flag.unwrap_or(0))]);
        }
    }

    #[test]
    fn small_getters_preserve_upper_bytes_edx_and_memory() {
        let mut memory = Object::new(0x140);
        let p = memory.ptr();
        unsafe {
            core::ptr::write_bytes(p, 0xA5, 0x140);
            put_pointer(p, 0x134, 0x1234_5678usize as *mut u8);
        }
        let before = unsafe { bytes(p, 0x140) };
        for (entry, want) in [
            (fc4p_00577450 as *const () as usize, (p as u32).wrapping_add(12)),
            (fc4p_0057f300 as *const () as usize, 0xC4C9_0997),
            (fc4p_00580b50 as *const () as usize, 0x1234_56A5),
            (fc4p_005a4de0 as *const () as usize, 0x1234_5678),
        ] {
            let mut call = RawCall::new(Conv::Thiscall, entry, p as u32, &[]);
            call.eax = 0x1234_5678;
            call.edx = 0xBEEF_1234;
            unsafe {
                call_raw(&mut call);
            }
            assert_eq!((call.eax, call.edx), (want, 0xBEEF_1234));
            assert_eq!(unsafe { bytes(p, 0x140) }, before);
        }
        for flag in [0, 1, 0x7F, 0x80, 0xFF] {
            unsafe {
                *p.add(0x1C) = flag;
            }
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_00580b50 as *const () as usize, p, &[], 0xDEAD_BE00).0 },
                0xDEAD_BE00 | u32::from(flag)
            );
        }
    }

    #[test]
    fn tagged_allocation_and_size_specific_vehicle_free() {
        let _tag = fake(0x0090_D91C, tag as *const () as usize);
        let _pool = fake(0x0055_F600, pooled as *const () as usize);
        let _free = fake(0x005E_5620, vehicle_delete as *const () as usize);
        let mut pool = 0u32;
        let _global = fake_data(0x00B0_A0F8, (&mut pool as *mut u32).cast());
        for size in [0, 1, 0x2B8, u32::MAX] {
            assert_eq!(
                unsafe { fc4p_0055b7c0(0x1234_5678, 0x8765_4321, size) },
                0xABC0_9876_5432_1000
            );
            assert_eq!(take_calls(), [Call::Tag(size, 16)]);
        }
        let storage = 0x8765_4321usize as *mut u8;
        for size in [0, 0x2B7, 0x2B8, 0x2B9, u32::MAX] {
            assert_eq!(unsafe { fc4p_0055ca60(core::ptr::null_mut(), size) }, 0);
            assert!(take_calls().is_empty());
            let result = unsafe { fc4p_0055ca60(storage, size) };
            if size == 0x2B8 {
                assert_eq!(result, 0x1234_ABCD);
                assert_eq!(
                    take_calls(),
                    [Call::VehiclePool((&mut pool as *mut u32) as usize, storage as usize)]
                );
            } else {
                assert_eq!(result, 0xABCD_1234);
                assert_eq!(take_calls(), [Call::VehicleFree(storage as usize)]);
            }
        }
    }

    #[test]
    fn deque_front_advances_or_retires_one_block_and_rereads_after_release() {
        let _free = fake(0x0090_CF63, delete as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        for boundary in [false, true] {
            for present in [false, true] {
                let mut memory = Object::new(0x28);
                let mut block = Object::new(0x80);
                let mut next = Object::new(0x80);
                let mut map = [block.ptr(), next.ptr()];
                let p = memory.ptr();
                let d = unsafe { EntryDeque::from_ptr(p) };
                let at = block.ptr().wrapping_add(if boundary { 0x78 } else { 0x10 });
                unsafe {
                    put_pointer(at, 0, if present { ip } else { core::ptr::null_mut() });
                }
                d.set_first(at);
                d.set_first_block(block.ptr());
                d.set_first_limit(block.ptr().wrapping_add(0x80));
                d.set_first_map(map.as_mut_ptr().cast());
                let mut expected = unsafe { bytes(p, 0x28) };
                let want = if boundary { next.ptr() } else { at.wrapping_add(8) };
                expected[..4].copy_from_slice(&(want as u32).to_le_bytes());
                if boundary {
                    expected[4..8].copy_from_slice(&(next.ptr() as u32).to_le_bytes());
                    expected[8..12].copy_from_slice(&(next.ptr().wrapping_add(0x80) as u32).to_le_bytes());
                    expected[12..16].copy_from_slice(&(map.as_mut_ptr().wrapping_add(1) as u32).to_le_bytes());
                }
                assert_eq!(unsafe { fc4p_0055faf0(p) }, want);
                let mut calls = Vec::new();
                if present {
                    calls.push(Call::Release(ip as usize));
                }
                if boundary {
                    calls.push(Call::Free(block.ptr() as usize));
                }
                assert_eq!(take_calls(), calls);
                assert_eq!(unsafe { bytes(p, 0x28) }, expected);
            }
        }
        let mut memory = Object::new(0x28);
        let mut block = Object::new(0x80);
        let d = unsafe { EntryDeque::from_ptr(memory.ptr()) };
        d.set_first(block.ptr());
        d.set_first_limit(block.ptr().wrapping_add(0x80));
        unsafe {
            put_pointer(block.ptr(), 0, ip);
        }
        mutation(
            Call::Release(ip as usize),
            memory.ptr(),
            block.ptr().wrapping_add(0x20) as u32,
        );
        assert_eq!(unsafe { fc4p_0055faf0(memory.ptr()) }, block.ptr().wrapping_add(0x28));
        assert_eq!(take_calls(), [Call::Release(ip as usize)]);
    }

    #[test]
    fn deque_append_reads_payload_after_retain_and_delegates_at_boundary() {
        let _grow = fake(0x0056_0250, block_append as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        let mut memory = Object::new(0x28);
        let mut block = Object::new(0x80);
        let d = unsafe { EntryDeque::from_ptr(memory.ptr()) };
        d.set_last(block.ptr());
        d.set_last_limit(block.ptr().wrapping_add(0x80));
        let mut value = [ip as u32, 0x1122_3344];
        mutation(
            Call::Retain(ip as usize),
            value.as_mut_ptr().wrapping_add(1).cast(),
            0x5566_7788,
        );
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) },
            0x5566_7788
        );
        assert_eq!(take_calls(), [Call::Retain(ip as usize)]);
        assert_eq!(
            unsafe { read_unaligned(block.ptr().cast::<[u32; 2]>()) },
            [ip as u32, 0x5566_7788]
        );
        assert_eq!(d.last(), block.ptr().wrapping_add(8));
        value = [0, 0xFEDC_BA98];
        assert_eq!(unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) }, value[1]);
        assert!(take_calls().is_empty());
        d.set_last(block.ptr().wrapping_add(0x78));
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), value.as_ptr().cast()) },
            0xFEDC_BA98
        );
        assert_eq!(
            take_calls(),
            [Call::AppendBlock(memory.ptr() as usize, value.as_ptr() as usize)]
        );
        d.set_last(core::ptr::null_mut());
        assert_eq!(
            unsafe { fc4p_005614b0(memory.ptr(), core::ptr::null()) },
            block.ptr().wrapping_add(0x78) as u32
        );
        assert_eq!(d.last(), 8usize as *mut u8);
    }

    #[test]
    fn deque_block_append_retains_temporary_and_grows_map_before_allocation() {
        let _allocate = fake(0x0090_CF54, allocation as *const () as usize);
        let _grow = fake(0x006D_B4A0, map_grow as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        for (capacity, present, destination_present) in
            [(3, true, true), (2, true, true), (3, false, true), (3, true, false)]
        {
            let mut memory = Object::new(0x28);
            let mut block = Object::new(0x80);
            let mut next = Object::new(0x80);
            let mut map = [block.ptr(), core::ptr::null_mut(), core::ptr::null_mut()];
            let p = memory.ptr();
            let d = unsafe { EntryDeque::from_ptr(p) };
            d.set_map(map.as_mut_ptr().cast());
            d.set_last_map(map.as_mut_ptr().wrapping_add(1).cast());
            d.set_map_capacity(capacity);
            let destination = if destination_present {
                block.ptr().wrapping_add(0x78)
            } else {
                core::ptr::null_mut()
            };
            d.set_last(destination);
            let mut value = [if present { ip as u32 } else { 0 }, 123];
            if present {
                mutation(
                    Call::Retain(ip as usize),
                    value.as_mut_ptr().wrapping_add(1).cast(),
                    456,
                );
            }
            ALLOCATION.with(|a| *a.borrow_mut() = next.ptr() as usize);
            let result = unsafe { fc4p_00560250(p, value.as_ptr().cast()) };
            let mut calls = Vec::new();
            if present {
                calls.push(Call::Retain(ip as usize));
            }
            if capacity == 2 {
                calls.push(Call::GrowMap(p as usize, 1, 0));
            }
            calls.push(Call::Allocate(0x80));
            if present && destination_present {
                calls.push(Call::Retain(ip as usize));
            }
            if present {
                calls.push(Call::Release(ip as usize));
            }
            assert_eq!(take_calls(), calls);
            assert_eq!(result, if present { 0x1122_3344 } else { next.ptr() as u32 });
            assert_eq!(map[2], next.ptr());
            assert_eq!(
                (d.last(), d.last_block(), d.last_limit(), d.last_map()),
                (
                    next.ptr(),
                    next.ptr(),
                    next.ptr().wrapping_add(0x80),
                    map.as_mut_ptr().wrapping_add(2).cast()
                )
            );
            if destination_present {
                assert_eq!(
                    unsafe { read_unaligned(destination.cast::<[u32; 2]>()) },
                    [value[0], if present { 456 } else { 123 }]
                );
            }
        }
    }

    #[test]
    fn vector_append_writes_before_retain_rereads_end_and_grows_with_zero_guard() {
        let _insert = fake(0x0043_0A90, vector_insert as *const () as usize);
        let table = table();
        let mut item = interface(&table);
        let ip = item.as_mut_ptr().cast::<u8>();
        let mut values = [0u32; 4];
        let mut vector = [
            values.as_mut_ptr() as usize,
            values.as_mut_ptr() as usize,
            values.as_mut_ptr().wrapping_add(4) as usize,
        ];
        let vp = vector.as_mut_ptr().cast::<u8>();
        WATCH.with(|w| *w.borrow_mut() = Some((values.as_mut_ptr() as usize, 16)));
        mutation(
            Call::Retain(ip as usize),
            unsafe { vp.add(4) },
            values.as_mut_ptr().wrapping_add(1) as u32,
        );
        assert_eq!(unsafe { fc4p_0059d7e0(vp, &ip) }, 0xAABB_CCDD);
        WATCH.with(|w| *w.borrow_mut() = None);
        assert_eq!(take_calls(), [Call::Retain(ip as usize)]);
        SNAPSHOTS.with(|s| assert_eq!(s.borrow_mut().pop().unwrap()[..4], (ip as u32).to_le_bytes()));
        assert_eq!(vector[1], values.as_mut_ptr().wrapping_add(2) as usize);
        let null = core::ptr::null_mut();
        unsafe {
            fc4p_0059d7e0(vp, &null);
        }
        assert_eq!(values[2], 0);
        assert!(take_calls().is_empty());
        vector[1] = vector[2];
        assert_eq!(unsafe { fc4p_0059d7e0(vp, &ip) }, 0x1357_9BDF);
        assert_eq!(
            take_calls(),
            [Call::Insert(vp as usize, vector[2], &ip as *const _ as usize, 0, 1, 1)]
        );
        vector[1] = 0;
        vector[2] = 4;
        unsafe {
            fc4p_0059d7e0(vp, core::ptr::null());
        }
        assert_eq!(vector[1], 4);
    }

    /// Captures ECX as well as EDX:EAX, and copies the actual argument slots back after return.
    pub(super) unsafe fn call_with_stack(call: &mut RawCall) {
        unsafe {
            core::arch::asm!(
                "mov ecx, [edi + 20]", "mov eax, [edi + 16]", "2:", "test ecx, ecx", "jz 3f",
                "dec ecx", "push [eax + ecx*4]", "jmp 2b", "3:",
                "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call [edi]",
                "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                "cmp dword ptr [edi + 24], 0", "jne 4f", "mov eax, [edi + 20]", "shl eax, 2", "sub esp, eax",
                "4:", "mov edx, [edi + 16]", "xor ecx, ecx", "5:", "cmp ecx, [edi + 20]", "je 6f",
                "mov eax, [esp + ecx*4]", "mov [edx + ecx*4], eax", "inc ecx", "jmp 5b", "6:",
                "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
                in("edi") call, clobber_abi("C"),
            );
        }
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn retain_registers(_this: *mut u8) -> u64 {
        core::arch::naked_asm!("call {body}", "mov ecx, 0x10203040", "ret", body = sym retain);
    }
    #[unsafe(naked)]
    unsafe extern "thiscall" fn release_registers(_this: *mut u8) -> u64 {
        core::arch::naked_asm!("call {body}", "mov ecx, 0x50607080", "ret", body = sym release);
    }

    #[test]
    fn interface_copy_keeps_registers_count_slot_and_retain_before_replace_before_release() {
        let table = [
            0,
            retain_registers as *const () as usize,
            release_registers as *const () as usize,
        ];
        let mut objects = [
            interface(&table),
            interface(&table),
            interface(&table),
            interface(&table),
        ];
        let a = objects[0].as_mut_ptr() as u32;
        let b = objects[1].as_mut_ptr() as u32;
        let c = objects[2].as_mut_ptr() as u32;
        let d = objects[3].as_mut_ptr() as u32;
        let mut source = [a, b, 0, a];
        let mut destination = [c, 0, d, a];
        let mut args = [
            source.as_mut_ptr() as u32,
            source.as_mut_ptr().wrapping_add(4) as u32,
            destination.as_mut_ptr() as u32,
        ];
        let mut call = RawCall::new(Conv::Cdecl, fc4p_00568ba0 as *const () as usize, 0x1357_2468, &args);
        call.args = args.as_mut_ptr();
        call.edx = 0xDEAD_BEEF;
        WATCH.with(|w| *w.borrow_mut() = Some((destination.as_mut_ptr() as usize, 16)));
        unsafe {
            call_with_stack(&mut call);
        }
        WATCH.with(|w| *w.borrow_mut() = None);
        assert_eq!(destination, source);
        assert_eq!(args[1], 0);
        assert_eq!(
            (call.eax, call.ecx, call.edx),
            (
                destination.as_mut_ptr().wrapping_add(4) as u32,
                0x5060_7080,
                0x8765_4321
            )
        );
        assert_eq!(
            take_calls(),
            [
                Call::Retain(a as usize),
                Call::Release(c as usize),
                Call::Retain(b as usize),
                Call::Release(d as usize)
            ]
        );
        SNAPSHOTS.with(|s| {
            let snapshots = std::mem::take(&mut *s.borrow_mut());
            assert_eq!(snapshots.len(), 4);
            assert_eq!(snapshots[0][..4], c.to_le_bytes());
            assert_eq!(snapshots[1][..4], a.to_le_bytes());
            assert_eq!(snapshots[2][4..8], 0u32.to_le_bytes());
            assert_eq!(snapshots[3][8..12], 0u32.to_le_bytes());
        });

        // Forward overlap observes values just written by the previous iteration.
        let mut overlap = [a, b, c];
        args = [
            overlap.as_mut_ptr() as u32,
            overlap.as_mut_ptr().wrapping_add(2) as u32,
            overlap.as_mut_ptr().wrapping_add(1) as u32,
        ];
        call.args = args.as_mut_ptr();
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(overlap, [a, a, a]);
        assert_eq!(
            take_calls(),
            [
                Call::Retain(a as usize),
                Call::Release(b as usize),
                Call::Retain(a as usize),
                Call::Release(c as usize)
            ]
        );
    }

    #[test]
    fn interface_copy_skips_identical_cells_values_and_nonpositive_signed_ranges() {
        let mut source = [0u32; 2];
        let start = source.as_mut_ptr() as u32;
        for (end, dest, expected_arg2) in [
            (start, 0x1234, start),
            (start.wrapping_sub(1), 0x1234, start.wrapping_sub(1)),
            (start + 8, start, 0),
        ] {
            let mut args = [start, end, dest];
            let mut call = RawCall::new(Conv::Cdecl, fc4p_00568ba0 as *const () as usize, 0x1357_2468, &args);
            call.args = args.as_mut_ptr();
            call.edx = 0xDEAD_BEEF;
            unsafe {
                call_with_stack(&mut call);
            }
            assert_eq!(args[1], expected_arg2);
            assert_eq!(call.eax, if dest == start { start + 8 } else { dest });
            assert_eq!((call.ecx, call.edx), (0x1357_2468, 0xDEAD_BEEF));
            assert!(take_calls().is_empty());
        }
        let mut dest = [0u32; 2];
        let mut args = [start, start + 8, dest.as_mut_ptr() as u32];
        let mut call = RawCall::new(Conv::Cdecl, fc4p_00568ba0 as *const () as usize, 0x1357_2468, &args);
        call.args = args.as_mut_ptr();
        call.edx = 0xDEAD_BEEF;
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!((call.ecx, call.edx), (0x1357_2468, 0xDEAD_BEEF));
        assert_eq!(args[1], 0);
    }

    #[test]
    fn route_list_clear_uses_cached_next_and_frees_arrays_then_interfaces_then_node() {
        let _free = fake(0x0090_CF63, delete as *const () as usize);
        let table = table();
        let mut a = interface(&table);
        let mut b = interface(&table);
        let ap = a.as_mut_ptr().cast::<u8>();
        let bp = b.as_mut_ptr().cast::<u8>();
        let mut owner = [0usize, 0xA5A5_A5A5];
        let op = owner.as_mut_ptr().cast::<u8>();
        let mut head = Object::new(16);
        let mut n1 = Object::new(0x34);
        let mut n2 = Object::new(0x34);
        unsafe {
            put_pointer(op, 0, head.ptr());
            put_pointer(head.ptr(), 0, n1.ptr());
            put_pointer(head.ptr(), 4, n2.ptr());
            put_pointer(n1.ptr(), 0, n2.ptr());
            put_pointer(n2.ptr(), 0, head.ptr());
            put_pointer(n1.ptr(), 0x30, 0x1110usize as *mut u8);
            put_pointer(n1.ptr(), 0x24, 0x2220usize as *mut u8);
            put_pointer(n1.ptr(), 0xC, ap);
            put_pointer(n1.ptr(), 8, bp);
        }
        let n1_before = unsafe { bytes(n1.ptr(), 0x34) };
        let n2_before = unsafe { bytes(n2.ptr(), 0x34) };
        mutation(Call::Free(0x1110), n1.ptr(), head.ptr() as u32);
        assert_eq!(unsafe { fc4p_00570c50(op) }, head.ptr());
        assert_eq!(
            take_calls(),
            [
                Call::Free(0x1110),
                Call::Free(0x2220),
                Call::Release(ap as usize),
                Call::Release(bp as usize),
                Call::Free(n1.ptr() as usize),
                Call::Free(n2.ptr() as usize)
            ]
        );
        assert_eq!(owner[1], 0xA5A5_A5A5);
        assert_eq!(unsafe { pointer(head.ptr(), 0) }, head.ptr());
        assert_eq!(unsafe { pointer(head.ptr(), 4) }, head.ptr());
        assert_eq!(unsafe { bytes(n1.ptr().wrapping_add(4), 0x30) }, n1_before[4..]);
        assert_eq!(unsafe { bytes(n2.ptr(), 0x34) }, n2_before);
        assert_eq!(unsafe { fc4p_00570c50(op) }, head.ptr());
        assert!(take_calls().is_empty());
    }

    #[test]
    fn interface_list_clear_keeps_edx_and_resets_only_the_two_sentinel_links() {
        let _free = fake(0x0090_CF63, delete as *const () as usize);
        let table = table();
        let mut object = interface(&table);
        let ip = object.as_mut_ptr().cast::<u8>();
        let mut head = Object::new(16);
        let mut node = Object::new(16);
        let mut owner = [head.ptr() as usize, 0xA5A5_A5A5];
        let op = owner.as_mut_ptr().cast::<u8>();
        for populated in [false, true] {
            unsafe {
                put_pointer(head.ptr(), 0, if populated { node.ptr() } else { head.ptr() });
                put_pointer(head.ptr(), 4, if populated { node.ptr() } else { head.ptr() });
                put_pointer(node.ptr(), 0, head.ptr());
                put_pointer(node.ptr(), 12, ip);
            }
            let node_before = unsafe { bytes(node.ptr(), 16) };
            let result = unsafe { fc4p_00584210(op, 0xDEAD_BEEF) };
            assert_eq!(result as u32, head.ptr() as u32);
            assert_eq!((result >> 32) as u32, if populated { 0x7654_3210 } else { 0xDEAD_BEEF });
            let expected = if populated {
                vec![Call::Release(ip as usize), Call::Free(node.ptr() as usize)]
            } else {
                vec![]
            };
            assert_eq!(take_calls(), expected);
            assert_eq!(unsafe { bytes(node.ptr(), 16) }, node_before);
            assert_eq!(unsafe { pointer(head.ptr(), 0) }, head.ptr());
            assert_eq!(unsafe { pointer(head.ptr(), 4) }, head.ptr());
            assert_eq!(owner[1], 0xA5A5_A5A5);
        }
    }

    extern "C" fn pool_allocation(size: u32) -> *mut u8 {
        log(Call::PoolAllocate(size));
        ALLOCATION.with(|a| *a.borrow() as *mut u8)
    }
    extern "thiscall" fn pool_grow(this: *mut u8) -> u64 {
        log(Call::PoolGrow(this as usize));
        let slot = ALLOCATION.with(|a| *a.borrow() as *mut u8);
        unsafe {
            put_pointer(this, 12, slot);
        }
        0x2468_ACE0_1357_9BDF
    }

    #[test]
    fn fixed_pool_growth_links_chunks_and_slots_without_touching_padding() {
        let _allocate = fake(0x009F_066E, pool_allocation as *const () as usize);
        for (stride, capacity) in [(4, 4), (8, 24), (12, 38), (16, 32)] {
            let mut chunk = Object::new(64);
            unsafe {
                core::ptr::write_bytes(chunk.ptr(), 0xA5, 64);
            }
            let mut pool = [stride, capacity, 0x1234_5678, 0xFFFF_FFFF, 0xA5A5_A5A5];
            let p = pool.as_mut_ptr().cast::<u8>();
            ALLOCATION.with(|a| *a.borrow_mut() = chunk.ptr() as usize);
            let result = unsafe { fc4p_005a61b0(p) };
            let first = chunk.ptr().wrapping_add(4);
            let last = first.wrapping_add(((capacity / stride - 1) * stride) as usize);
            assert_eq!(result, (u64::from(last as u32) << 32) | u64::from(last as u32));
            let mut expected = vec![0xA5; 64];
            expected[..4].copy_from_slice(&0x1234_5678u32.to_le_bytes());
            let mut offset = 4;
            while offset < 4 + ((capacity / stride - 1) * stride) as usize {
                expected[offset..offset + 4]
                    .copy_from_slice(&(chunk.ptr().wrapping_add(offset + stride as usize) as u32).to_le_bytes());
                offset += stride as usize;
            }
            expected[offset..offset + 4].fill(0);
            assert_eq!(unsafe { bytes(chunk.ptr(), 64) }, expected);
            assert_eq!(pool, [stride, capacity, chunk.ptr() as u32, first as u32, 0xA5A5_A5A5]);
            assert_eq!(take_calls(), [Call::PoolAllocate(capacity + 8)]);
        }
    }

    #[test]
    fn pool_slot_returns_ecx_link_and_preserves_edx_until_growth() {
        let _grow = fake(0x005A_61B0, pool_grow as *const () as usize);
        let mut slots = [0x1234_5678u32, 0xA5A5_A5A5];
        for empty in [false, true] {
            let mut pool = [8, 24, 0x1357, if empty { 0 } else { slots.as_mut_ptr() as u32 }];
            let p = pool.as_mut_ptr().cast::<u8>();
            ALLOCATION.with(|a| *a.borrow_mut() = slots.as_mut_ptr() as usize);
            let mut call = RawCall::new(Conv::Thiscall, fc4p_005a6290 as *const () as usize, p as u32, &[]);
            call.edx = 0xDEAD_BEEF;
            unsafe {
                call_with_stack(&mut call);
            }
            assert_eq!(
                (call.eax, call.ecx, call.edx),
                (
                    slots.as_mut_ptr() as u32,
                    0x1234_5678,
                    if empty { 0x2468_ACE0 } else { 0xDEAD_BEEF }
                )
            );
            assert_eq!(pool[3], 0x1234_5678);
            assert_eq!(slots, [0x1234_5678, 0xA5A5_A5A5]);
            assert_eq!(
                take_calls(),
                if empty {
                    vec![Call::PoolGrow(p as usize)]
                } else {
                    vec![]
                }
            );
        }
    }

    extern "thiscall" fn position(this: *mut u8, output: *mut u32) {
        log(Call::Position(this as usize));
        unsafe {
            for i in 0..3 {
                write_unaligned(output.add(i), read_unaligned(this.add(4 + i * 4).cast()));
            }
        }
    }
    extern "thiscall" fn radius(this: *mut u8) -> f32 {
        log(Call::Radius(this as usize));
        unsafe { read_unaligned(this.add(16).cast()) }
    }

    #[test]
    fn proximity_search_skips_null_and_ignored_uses_strict_radius_and_writes_distance_argument() {
        let _fpu = GameFpu::new();
        let mut scale = 2.0f32;
        let mut coordinate = 0u32;
        let _scale = fake_data(0x00B0_A158, (&mut scale as *mut f32).cast());
        let _coordinate = fake_data(0x00B2_2A80, (&mut coordinate as *mut u32).cast());
        let mut table = [0usize; 18];
        table[0x1C / 4] = position as *const () as usize;
        table[0x44 / 4] = radius as *const () as usize;
        let mut a = [
            table.as_ptr() as u32,
            3.0f32.to_bits(),
            4.0f32.to_bits(),
            0,
            2.5f32.to_bits(),
        ];
        let mut b = [table.as_ptr() as u32, 0, 0, 0, 1.0f32.to_bits()];
        let ap = a.as_mut_ptr().cast::<u8>();
        let bp = b.as_mut_ptr().cast::<u8>();
        let mut head = [0usize; 3];
        let mut n3 = [head.as_mut_ptr() as usize, 0, bp as usize];
        let mut n2 = [n3.as_mut_ptr() as usize, 0, ap as usize];
        let mut n1 = [n2.as_mut_ptr() as usize, 0, 0];
        head[0] = n1.as_mut_ptr() as usize;
        let mut owner = Object::new(0x48);
        unsafe {
            put_pointer(owner.ptr(), 0x44, head.as_mut_ptr().cast());
        }
        let mut args = [0u32; 4];
        let mut call = RawCall::new(
            Conv::Thiscall,
            fc4p_00568c10 as *const () as usize,
            owner.ptr() as u32,
            &args,
        );
        call.args = args.as_mut_ptr();
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(call.eax, bp as u32);
        assert_eq!(args[3], 0.0f32.to_bits());
        assert_eq!(
            take_calls(),
            [
                Call::Position(ap as usize),
                Call::Radius(ap as usize),
                Call::Position(bp as usize),
                Call::Radius(bp as usize)
            ]
        );
        args[3] = bp as u32;
        call.ecx = owner.ptr() as u32;
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(call.eax, 0);
        assert_eq!(args[3], 5.0f32.to_bits());
        assert_eq!(take_calls(), [Call::Position(ap as usize), Call::Radius(ap as usize)]);
        unsafe {
            write_unaligned(ap.add(16).cast::<u32>(), f32::NAN.to_bits());
        }
        args[3] = bp as u32;
        call.ecx = owner.ptr() as u32;
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(call.eax, 0);
        assert_eq!(args[3], 5.0f32.to_bits());
        take_calls();
        unsafe {
            head.as_mut_ptr().write(head.as_mut_ptr() as usize);
        }
        args[3] = 0xDEAD_BEEF;
        call.ecx = owner.ptr() as u32;
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(call.eax, 0);
        assert_eq!(args[3], 0xDEAD_BEEF);
        assert!(take_calls().is_empty());
    }

    #[test]
    fn wave_displacement_uses_initial_point_and_rounds_only_y_and_z_contributions() {
        let _fpu = GameFpu::new();
        let mut records = [[0.0f32, 0., 0., 0., 1., 1., 1.]; 2];
        records[0] = [0., 0., 0., 1., 3., 4., 5.];
        records[1] = [0., 0., 0., 1., 7., 8., 9.];
        let mut vector = [
            records.as_mut_ptr() as usize,
            records.as_mut_ptr().wrapping_add(2) as usize,
        ];
        let mut point = [1.0f32, 2., 0.5];
        let wave = F80::from_f32(0.5).sin();
        let mut expected = point;
        for record in records {
            expected[0] = add_float(multiply_float(wave, record[4]), expected[0]).to_f32();
            for i in [1, 2] {
                expected[i] =
                    add_float(F80::from_f32(multiply_float(wave, record[4 + i]).to_f32()), expected[i]).to_f32();
            }
        }
        unsafe {
            fc4p_005ac070(0., vector.as_mut_ptr().cast(), point.as_mut_ptr());
        }
        assert_eq!(point.map(f32::to_bits), expected.map(f32::to_bits));
        vector[1] = vector[0];
        let original = point;
        unsafe {
            fc4p_005ac070(f32::NAN, vector.as_mut_ptr().cast(), point.as_mut_ptr());
        }
        assert_eq!(point.map(f32::to_bits), original.map(f32::to_bits));

        // The register contribution stays finite in x after cancellation; stored y/z overflow.
        let amplitude = f32::from_bits(0x6000_0000);
        records[0] = [1., 0., 0., 0., amplitude, amplitude, amplitude];
        vector[1] = records.as_mut_ptr().wrapping_add(1) as usize;
        point = [f32::from_bits(0xFF00_0000); 3];
        unsafe {
            fc4p_005ac070(
                f32::from_bits(0x5F00_0000),
                vector.as_mut_ptr().cast(),
                point.as_mut_ptr(),
            );
        }
        assert_eq!(point[0].to_bits(), 0x7F00_0000);
        assert_eq!(point[1].to_bits(), f32::INFINITY.to_bits());
        assert_eq!(point[2].to_bits(), f32::INFINITY.to_bits());
    }

    extern "thiscall" fn hash_bucket(this: *mut u8, key: *const u8) -> u32 {
        log(Call::Hash(this as usize, key as usize));
        BUCKET.with(|b| *b.borrow())
    }

    #[test]
    fn byte_key_range_matches_binary_keys_and_contiguous_duplicates_and_scans_later_buckets() {
        let _hash = fake(0x0051_C9B0, hash_bucket as *const () as usize);
        let mut key_bytes = [0x80u8, 0, 0xFF];
        let mut other_bytes = [0x80u8, 0, 0xFE];
        let begin = key_bytes.as_mut_ptr() as usize;
        let end = key_bytes.as_mut_ptr().wrapping_add(3) as usize;
        let mut key = [begin, end];
        let mut nodes = [[0usize; 3]; 6];
        let addresses: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as usize).collect();
        // A short key and a same-length mismatch precede a group of two equal keys.
        nodes[0] = [addresses[1], begin, end - 1];
        nodes[1] = [
            addresses[2],
            other_bytes.as_mut_ptr() as usize,
            other_bytes.as_mut_ptr().wrapping_add(3) as usize,
        ];
        nodes[2] = [addresses[3], begin, end];
        nodes[3] = [addresses[4], begin, end];
        nodes[4] = [
            0,
            other_bytes.as_mut_ptr() as usize,
            other_bytes.as_mut_ptr().wrapping_add(3) as usize,
        ];
        unsafe {
            nodes.as_mut_ptr().add(5).write([0, begin, end]);
        }
        let mut buckets = [addresses[0], 0, 0, addresses[5]];
        let mut map = [
            0xA5A5_A5A5,
            buckets.as_mut_ptr() as usize,
            buckets.as_mut_ptr().wrapping_add(4) as usize,
        ];
        let mp = map.as_mut_ptr().cast::<u8>();
        let mut output = [0xDEAD_BEEFusize; 6];
        for (mode, first, after, key_slot) in [
            (0, addresses[2], addresses[4], begin),
            (1, addresses[2], addresses[5], begin),
            (2, addresses[2], 0, begin),
            (3, 0, 0, begin),
            (4, 0, 0, key.as_mut_ptr() as usize),
        ] {
            match mode {
                1 => unsafe {
                    (addresses[3] as *mut usize).write(0);
                },
                2 => unsafe {
                    buckets.as_mut_ptr().add(3).write(0);
                },
                3 => unsafe {
                    (addresses[1] as *mut usize).write(0);
                },
                4 => unsafe {
                    buckets.as_mut_ptr().write(0);
                },
                _ => {}
            }
            let mut args = [output.as_mut_ptr().wrapping_add(1) as u32, key.as_mut_ptr() as u32];
            let mut call = RawCall::new(Conv::Thiscall, fc4p_00586ef0 as *const () as usize, mp as u32, &args);
            call.args = args.as_mut_ptr();
            unsafe {
                call_with_stack(&mut call);
            }
            assert_eq!(call.eax, output.as_mut_ptr().wrapping_add(1) as u32);
            assert_eq!(
                output,
                [0xDEAD_BEEF, first, mp as usize, after, mp as usize, 0xDEAD_BEEF]
            );
            assert_eq!(args[1], key_slot as u32);
            assert_eq!(take_calls(), [Call::Hash(mp as usize, key.as_mut_ptr() as usize)]);
        }
        assert_eq!(map[0], 0xA5A5_A5A5);
    }

    #[test]
    fn empty_byte_keys_match_without_reading_storage_and_hash_callback_changes_are_observed() {
        let _hash = fake(0x0051_C9B0, hash_bucket as *const () as usize);
        let mut node = [0usize; 3];
        let mut buckets = [node.as_mut_ptr() as usize];
        let mut old_buckets = [0usize];
        let mut map = [
            0usize,
            old_buckets.as_mut_ptr() as usize,
            buckets.as_mut_ptr().wrapping_add(1) as usize,
        ];
        let mp = map.as_mut_ptr().cast::<u8>();
        let mut key = [0usize; 2];
        let kp = key.as_mut_ptr();
        let mut result = [0usize; 4];
        mutation(
            Call::Hash(mp as usize, kp as usize),
            unsafe { mp.add(4) },
            buckets.as_mut_ptr() as u32,
        );
        let mut args = [result.as_mut_ptr() as u32, kp as u32];
        let mut call = RawCall::new(Conv::Thiscall, fc4p_00586ef0 as *const () as usize, mp as u32, &args);
        call.args = args.as_mut_ptr();
        unsafe {
            call_with_stack(&mut call);
        }
        assert_eq!(result, [node.as_mut_ptr() as usize, mp as usize, 0, mp as usize]);
        assert_eq!(args[1], 0);
        assert_eq!(take_calls(), [Call::Hash(mp as usize, kp as usize)]);
    }

    #[unsafe(naked)]
    unsafe extern "C" fn allocation_registers(_size: u32, _tag: u32) -> u64 {
        core::arch::naked_asm!("xor eax, ecx", "add eax, [esp + 4]", "add eax, [esp + 8]", "ret");
    }

    #[test]
    fn tagged_allocation_supplies_the_original_input_registers_and_keeps_edx() {
        let _allocation = fake(0x0090_D91C, allocation_registers as *const () as usize);
        let args = [0x1357_2468];
        let mut call = RawCall::new(Conv::Stdcall, fc4p_0055b7c0 as *const () as usize, 0x2468_1357, &args);
        call.eax = 0xABCD_EF01;
        call.edx = 0xDEAD_BEEF;
        unsafe {
            call_raw(&mut call);
        }
        assert_eq!(
            call.eax,
            (args[0] ^ 0x2468_1357u32).wrapping_add(args[0]).wrapping_add(16)
        );
        assert_eq!(call.edx, 0xDEAD_BEEF);
    }

    pub(super) fn float_environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        unsafe {
            core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]",
            "pushfd", "pop {flags:e}", cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            flags = lateout(reg) flags, out("ax") sw);
        }
        (cw, sw & 0x3800, mxcsr, flags & 0x400)
    }

    #[test]
    fn wave_phase_order_nan_payloads_and_float_environment() {
        let _fpu = GameFpu::new();
        let mut record = [1.0f32, -16777216., 0.5, 16777216., 1., 2., 3.];
        let mut vector = [
            record.as_mut_ptr() as usize,
            record.as_mut_ptr().wrapping_add(7) as usize,
        ];
        let mut point = [1.0f32; 3];
        let sentinel = 123.25f64;
        let mut restored = 0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        let before = float_environment();
        unsafe {
            fc4p_005ac070(0.25, vector.as_mut_ptr().cast(), point.as_mut_ptr());
        }
        let after = float_environment();
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(before, after);
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        assert_eq!(point.map(f32::to_bits), [0x3FD7_3FF0, 0x4017_3FF0, 0x4042_DFE9]);
        record = [0., 1., 0., 0., 1., 2., 3.];
        std::hint::black_box(&record);
        for input in [0x7F80_1234, 0xFF80_1234, 0x7FC0_1234, 0xFFC0_1234] {
            point = [f32::from_bits(input), 0., 0.];
            unsafe {
                fc4p_005ac070(0., vector.as_mut_ptr().cast(), point.as_mut_ptr());
            }
            assert_eq!(point.map(f32::to_bits), [input | 0x0040_0000; 3]);
        }
    }
}
