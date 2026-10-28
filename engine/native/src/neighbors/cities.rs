// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Adjacent and directly connected regional-city lists.

use super::{pair, pointer, virtual_call, word};
use crate::abi::{call_raw, Conv, RawCall};
use crate::port::Port;

game_fn! { fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { fn release_local(local: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1870; }
game_fn! { fn clear_cities(list: *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1DF0; }
game_fn! { fn clear_connections(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1D70; }
game_fn! { fn city_node(list: *mut u8, edx: u32, count: u32, hint: u32) -> u64 = "fastcall" 0x0041_1B60; }
game_fn! { fn allocate_tree(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { fn insert_city(tree: *mut CitySet, edx: u32, out: *mut u32, city: *mut *mut u8) -> u64 = "fastcall" 0x0049_A050; }
game_fn! { fn erase_subtree(tree: *mut CitySet, edx: u32, root: *mut u8) -> u64 = "fastcall" 0x004A_4660; }
// The raw call below also supplies the register words carried into the game's free helper.
game_fn! { #[allow(dead_code)] fn free_tree(memory: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_data! { static city_allocator: *mut u8 = 0x00B2_07E0; }
game_data! { static connection_allocator: *mut u8 = 0x00B2_07DC; }
game_data! { static region_manager: *mut u8 = 0x00B4_3C94; }

#[repr(C)]
struct CitySet {
    header: *mut u8,
    count: u32,
}

layout! {
    /// Ordered city-pointer tree. Null child links distinguish leaves from the sentinel.
    struct CityNode {
        0x00 => color / set_color: u8,
        0x04 => parent / set_parent: *mut u8,
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => city / set_city: *mut u8,
    }
}

pub(super) unsafe fn service(cache: *mut *mut u8, scratch: *mut *mut u8, edx: &mut u32) -> *mut u8 {
    unsafe {
        if cache.read().is_null() {
            scratch.write_unaligned(core::ptr::null_mut());
            let result = framework();
            let framework = result as u32 as *mut u8;
            if !framework.is_null() {
                virtual_call(
                    framework,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, scratch as u32],
                    framework as u32,
                    word(framework, 0),
                );
            }
            cache.write(scratch.read_unaligned());
            let result = release_local(scratch, scratch.read_unaligned() as u32);
            *edx = (result >> 32) as u32;
        }
        cache.read()
    }
}

unsafe fn free(memory: *mut u8, eax: u32, edx: u32, ecx: u32) -> u64 {
    let args = [memory as u32];
    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), ecx, &args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    pair(call.eax, call.edx)
}

unsafe fn insert_at(region: *mut u8, cell: [i32; 2], along_x: bool, include: u8, tree: &mut CitySet, edx: &mut u32) {
    unsafe {
        let [x, z] = cell;
        let result = virtual_call(
            region,
            0x2C,
            &[x as u32, z as u32],
            if along_x { z as u32 } else { x as u32 },
            word(region, 0),
        );
        *edx = (result >> 32) as u32;
        let reference = result as u32 as *mut u8;
        if reference.is_null() {
            return;
        }
        let city = pointer(reference, 0);
        if city.is_null() {
            return;
        }
        if include == 0 {
            let result = virtual_call(city, 0xAC, &[], word(city, 0), *edx);
            *edx = (result >> 32) as u32;
            if result as u8 == 0 {
                return;
            }
        }
        let mut city = pointer(reference, 0);
        let mut out = [0u32; 2];
        let result = insert_city(tree, &mut city as *mut *mut u8 as u32, out.as_mut_ptr(), &mut city);
        *edx = (result >> 32) as u32;
    }
}

unsafe fn successor(node: *mut u8) -> *mut u8 {
    unsafe {
        let right = pointer(node, 0x0C);
        if !right.is_null() {
            let mut next = right;
            while !pointer(next, 8).is_null() {
                next = pointer(next, 8);
            }
            next
        } else {
            let mut next = node;
            let mut parent = pointer(node, 4);
            while next == pointer(parent, 0x0C) {
                next = parent;
                parent = pointer(parent, 4);
            }
            if pointer(next, 0x0C) != parent {
                next = parent;
            }
            next
        }
    }
}

port! {
    /// Clears the output, visits the cells beyond all four edges, and deduplicates cities in a temporary
    /// pointer-ordered tree. Copies its cities to the output, then frees the tree without releasing cities.
    /// A first allocator lookup can overwrite the incoming include word after the edge scan has finished.
    #[port(address = 0x006D_01E0, owner = "neighbors", name = "cSC4NeighborsSimulator::GetAdjacentCities", naked)]
    pub unsafe extern "C" fn fc4p_006d01e0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 8", sym adjacent);
    }
}

unsafe extern "fastcall" fn adjacent(_this: *mut u8, args: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let list = args.read_unaligned() as *mut u8;
        clear_cities(list, entry_edx);
        let result = allocate_tree(20);
        let header = result as u32 as *mut u8;
        let node = CityNode::from_ptr(header);
        node.set_color(0);
        node.set_parent(core::ptr::null_mut());
        node.set_left(header);
        node.set_right(header);
        let mut tree = CitySet { header, count: 0 };
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x8C, &[], header as u32, word(manager, 0));
        let region = result as u32 as *mut u8;
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], word(manager, 0), (result >> 32) as u32);
        let city = result as u32 as *mut u8;
        let mut position = [0i32; 2];
        let mut size = [0i32; 2];
        virtual_call(
            city,
            0x14,
            &[position.as_mut_ptr() as u32, position.as_mut_ptr().add(1) as u32],
            position.as_mut_ptr().add(1) as u32,
            word(city, 0),
        );
        let result = virtual_call(
            city,
            0x1C,
            &[size.as_mut_ptr() as u32, size.as_mut_ptr().add(1) as u32],
            size.as_mut_ptr().add(1) as u32,
            word(city, 0),
        );
        let mut edx = (result >> 32) as u32;
        let include = args.add(1).read_unaligned() as u8;
        let mut i = 0i32;
        while i < size[0] {
            let x = position[0].wrapping_add(i);
            insert_at(
                region,
                [x, position[1].wrapping_sub(1)],
                true,
                include,
                &mut tree,
                &mut edx,
            );
            insert_at(
                region,
                [x, position[1].wrapping_add(size[1])],
                true,
                include,
                &mut tree,
                &mut edx,
            );
            i = i.wrapping_add(1);
        }
        i = 0;
        while i < size[1] {
            let z = position[1].wrapping_add(i);
            insert_at(
                region,
                [position[0].wrapping_sub(1), z],
                false,
                include,
                &mut tree,
                &mut edx,
            );
            insert_at(
                region,
                [position[0].wrapping_add(size[0]), z],
                false,
                include,
                &mut tree,
                &mut edx,
            );
            i = i.wrapping_add(1);
        }
        let mut cursor = pointer(tree.header, 8);
        while cursor != tree.header {
            let city = pointer(cursor, 0x10);
            if !city.is_null() {
                let result = virtual_call(city, 4, &[], cursor as u32, word(city, 0));
                edx = (result >> 32) as u32;
            }
            let destination_header = pointer(list, 0);
            let allocator = service(city_allocator(), args.add(1).cast(), &mut edx);
            let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
            let allocated = result as u32 as *mut u8;
            edx = (result >> 32) as u32;
            let data = allocated.wrapping_add(8);
            if !data.is_null() {
                data.cast::<*mut u8>().write_unaligned(city);
                if !city.is_null() {
                    let result = virtual_call(city, 4, &[], data as u32, word(city, 0));
                    edx = (result >> 32) as u32;
                }
            }
            let previous = pointer(destination_header, 4);
            allocated.cast::<*mut u8>().write_unaligned(destination_header);
            allocated.add(4).cast::<*mut u8>().write_unaligned(previous);
            previous.cast::<*mut u8>().write_unaligned(allocated);
            destination_header.add(4).cast::<*mut u8>().write_unaligned(allocated);
            if !city.is_null() {
                let result = virtual_call(city, 8, &[], word(city, 0), edx);
                edx = (result >> 32) as u32;
            }
            cursor = successor(cursor);
        }
        let mut final_eax = tree.count;
        if tree.count != 0 {
            let mut root = pointer(tree.header, 4);
            while !root.is_null() {
                let right = pointer(root, 0x0C);
                let result = erase_subtree(&mut tree, edx, right);
                edx = (result >> 32) as u32;
                let left = pointer(root, 8);
                let result = free(root, result as u32, edx, &mut tree as *mut CitySet as u32);
                edx = (result >> 32) as u32;
                root = left;
            }
            let node = CityNode::from_ptr(tree.header);
            node.set_left(tree.header);
            node.set_parent(core::ptr::null_mut());
            node.set_right(tree.header);
            tree.count = 0;
            edx = tree.header as u32;
            final_eax = tree.header as u32;
        }
        if tree.header.is_null() {
            pair(final_eax, edx)
        } else {
            free(tree.header, final_eax, edx, tree.header as u32)
        }
    }
}

port! {
    /// Copies transport-connected cities in connection order, testing establishment before deduplication.
    /// Each accepted city takes a temporary reference around the output scan, including duplicates.
    #[port(address = 0x006D_1E00, owner = "neighbors", name = "cSC4NeighborsSimulator::GetConnectedCities", naked)]
    pub unsafe extern "C" fn fc4p_006d1e00() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 8", sym connected);
    }
}

unsafe extern "fastcall" fn connected(this: *mut u8, args: *mut u32, entry_edx: u32) -> u64 {
    unsafe {
        let list = args.read_unaligned() as *mut u8;
        let result = clear_cities(list, entry_edx);
        let mut edx = (result >> 32) as u32;
        let mut scratch = core::ptr::null_mut();
        let allocator = service(connection_allocator(), &mut scratch, &mut edx);
        let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
        let mut header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        let result = virtual_call(
            this,
            0x40,
            &[&mut header as *mut *mut u8 as u32],
            &mut header as *mut *mut u8 as u32,
            word(this, 0),
        );
        edx = (result >> 32) as u32;
        let mut node = pointer(header, 0);
        while node != header {
            let connection = pointer(node, 8);
            let mut result = virtual_call(connection, 0x14, &[], node as u32, word(connection, 0));
            if matches!(result as u32, 1..=3 | 6..=12) {
                result = virtual_call(connection, 0x24, &[], 0, word(connection, 0));
                let city = result as u32 as *mut u8;
                if !city.is_null() {
                    let include = args.add(1).read_unaligned() as u8;
                    let established = if include != 0 {
                        result = pair((result as u32 & !0xFF) | u32::from(include), (result >> 32) as u32);
                        true
                    } else {
                        result = virtual_call(city, 0xAC, &[], word(city, 0), (result >> 32) as u32);
                        result as u8 != 0
                    };
                    if established {
                        result = virtual_call(city, 4, &[], result as u32, word(city, 0));
                        let destination_header = pointer(list, 0);
                        let mut existing = pointer(destination_header, 0);
                        while existing != destination_header && pointer(existing, 8) != city {
                            existing = pointer(existing, 0);
                        }
                        result = virtual_call(city, 8, &[], word(city, 0), (result >> 32) as u32);
                        if existing == pointer(list, 0) {
                            result = virtual_call(city, 4, &[], result as u32, word(city, 0));
                            let destination_header = pointer(list, 0);
                            result = city_node(list, (result >> 32) as u32, 1, 0);
                            let allocated = result as u32 as *mut u8;
                            let data = allocated.wrapping_add(8);
                            if !data.is_null() {
                                data.cast::<*mut u8>().write_unaligned(city);
                                virtual_call(city, 4, &[], word(city, 0), (result >> 32) as u32);
                            }
                            let previous = pointer(destination_header, 4);
                            allocated.cast::<*mut u8>().write_unaligned(destination_header);
                            allocated.add(4).cast::<*mut u8>().write_unaligned(previous);
                            previous.cast::<*mut u8>().write_unaligned(allocated);
                            destination_header.add(4).cast::<*mut u8>().write_unaligned(allocated);
                            result = virtual_call(city, 8, &[], previous as u32, word(city, 0));
                        }
                    }
                }
            }
            edx = (result >> 32) as u32;
            node = pointer(node, 0);
        }
        let result = clear_connections(&mut header, edx);
        edx = (result >> 32) as u32;
        let saved_header = header;
        let allocator = service(connection_allocator(), args.add(1).cast(), &mut edx);
        virtual_call(allocator, 0x10, &[saved_header as u32], word(allocator, 0), edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d01e0::PORT, fc4p_006d1e00::PORT];

#[cfg(test)]
mod tests;
