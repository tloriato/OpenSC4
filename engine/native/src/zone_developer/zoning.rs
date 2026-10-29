// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Zoning orchestration, component selection and the preview's remaining cells.

use super::{region::Region, Port, ZoneDeveloper};
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// Options and the optional cell used to select a preview component.
    struct Selection {
        0x9C => custom_size / set_custom_size: u8,
        0xA8 => point / set_point: *const i32,
    }
}

game_fn! { fn examine(this: *mut u8) = "thiscall" 0x0073_1C80; }
game_fn! { fn allocate_node(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free_node(node: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn allocate_region(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn free_region(region: *mut u8) = "C" 0x005E_5620; }
game_fn! { fn construct_region(this: *mut u8, source: *mut u8, fill: u32) -> *mut u8 = "thiscall" 0x0044_2640; }
game_fn! { fn destroy_bits(this: *mut u8) = "thiscall" 0x0044_1A60; }
game_fn! { fn destroy_list(this: *mut *mut u8) -> u64 = "thiscall" 0x0056_50A0; }
game_fn! {
    #[allow(clippy::too_many_arguments)]
    fn face_road(this: *mut u8, x: i32, z: i32, width: i32, height: i32, orientations: *mut *mut u8, lot: *mut *mut u8) -> u32
        = "thiscall" 0x0072_D200;
}
game_fn! { fn allocate_message(size: u32) -> *mut u8 = "C" 0x0091_33DA; }
game_fn! { fn construct_message(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6; }
game_fn! { fn parcellizable(this: *mut u8, source: *mut u8) -> *mut u8 = "thiscall" 0x0072_CB90; }
game_fn! { #[allow(dead_code)] fn compress_region(region: *mut *mut u8) = "stdcall" 0x0072_AF30; }
game_fn! { fn lot_size(this: *mut u8, region: *mut u8) = "thiscall" 0x0073_2BF0; }
game_fn! { fn extend_streets(this: *mut u8, region: *mut u8, kind: u32) = "thiscall" 0x0073_0BA0; }
game_fn! { fn jog_streets(this: *mut u8, region: *mut u8, kind: u32) = "thiscall" 0x0073_0D30; }
game_fn! { fn subdivide(this: *mut u8, region: *mut u8, list: *mut *mut u8) = "thiscall" 0x0072_EC60; }
game_fn! { fn parcellize(this: *mut u8) = "thiscall" 0x0072_EDD0; }
game_fn! { fn simple_parcellize(this: *mut u8, depth: u32) = "thiscall" 0x0072_E9B0; }
game_fn! { fn extract(region: *mut u8, x: u32, z: u32) -> *mut u8 = "stdcall" 0x0072_CDA0; }
game_fn! { fn update_zone(this: *mut u8, source: *mut u8) = "thiscall" 0x0072_B180; }

unsafe fn compress(this: *mut u8, region: *mut *mut u8) {
    let args = [region as u32];
    let mut call = crate::abi::RawCall::new(
        crate::abi::Conv::Stdcall,
        crate::abi::target(0x0072_AF30),
        this as u32,
        &args,
    );
    unsafe {
        crate::abi::call_raw(&mut call);
    }
}

unsafe fn empty_list() -> *mut u8 {
    unsafe {
        let header = allocate_node(12);
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
        header
    }
}

unsafe fn append_orientation(header: *mut u8, orientation: u32) {
    unsafe {
        let node = allocate_node(12);
        let value = node.wrapping_add(8).cast::<u32>();
        if !value.is_null() {
            write_unaligned(value, orientation);
        }
        let previous = read_unaligned(header.add(4).cast::<*mut u8>());
        write_unaligned(node.cast::<*mut u8>(), header);
        write_unaligned(node.add(4).cast::<*mut u8>(), previous);
        write_unaligned(previous.cast::<*mut u8>(), node);
        write_unaligned(header.add(4).cast::<*mut u8>(), node);
    }
}

unsafe fn clear_nodes(header: *mut u8, mut result: u64) -> u64 {
    unsafe {
        let mut node = read_unaligned(header.cast::<*mut u8>());
        while node != header {
            let next = read_unaligned(node.cast::<*mut u8>());
            result = free_node_registers(node, node as u32, (result >> 32) as u32);
            node = next;
        }
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
        result
    }
}

unsafe fn free_node_registers(node: *mut u8, eax: u32, edx: u32) -> u64 {
    let args = [node as u32];
    let mut call = crate::abi::RawCall::new(crate::abi::Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        crate::abi::call_raw(&mut call);
    }
    (u64::from(call.edx) << 32) | u64::from(call.eax)
}

unsafe fn new_region(source: *mut u8) -> *mut u8 {
    unsafe {
        let allocation = allocate_region(0x24);
        if allocation.is_null() {
            null_mut()
        } else {
            construct_region(allocation, source, 0)
        }
    }
}

unsafe fn delete_region(region: *mut u8) {
    unsafe {
        if !region.is_null() {
            Region::from_ptr(region).set_vtable(0x00A8_6364);
            destroy_bits(region.add(16));
            free_region(region);
        }
    }
}

/// Component cleanup is inline: free every packed column, its pointer array and the region shell.
unsafe fn delete_component(region: *mut u8) {
    unsafe {
        if !region.is_null() {
            let region = Region::from_ptr(region);
            region.set_vtable(0x00A8_6364);
            if !region.columns().is_null() {
                let mut x = 0u32;
                while x < region.width() as u32 {
                    free_region(read_unaligned(region.columns().wrapping_add(x as usize)).cast());
                    x = x.wrapping_add(1);
                }
                free_region(region.columns().cast());
            }
            free_region(region.as_ptr());
        }
    }
}

unsafe fn defer(dev: ZoneDeveloper, value: u32) -> u64 {
    unsafe {
        let allocation = allocate_message(0x2C);
        let message = if allocation.is_null() {
            null_mut()
        } else {
            construct_message(allocation)
        };
        vcall!(message, 4 => fn());
        vcall!(message, 0x14 => fn(u32), 0x49D9_68C4);
        vcall!(message, 0x2C => fn(u32), value);
        let posted = vcall!(dev.message_server(), 0x10 => fn(*mut u8, u32) -> u32, message, 0);
        super::virtual_registers(message, 8, posted, read_unaligned(message.cast::<u32>()), &[])
    }
}

/// Test the centre first. For an absent centre, query both city dimensions even if a near cell matches.
unsafe fn selected(dev: ZoneDeveloper, component: Region, point: *const i32) -> bool {
    unsafe {
        let x = read_unaligned(point);
        let z = read_unaligned(point.add(1));
        if component.includes(x, z) {
            return true;
        }
        let mut near = x > 0 && component.includes(x.wrapping_sub(1), z);
        let cells_x = vcall!(dev.city(), 0x174 => fn() -> i32);
        let current = Region::from_ptr(dev.restriction());
        if x < cells_x.wrapping_sub(1) && current.includes(x.wrapping_add(1), z) {
            near = true;
        }
        let current = Region::from_ptr(dev.restriction());
        if z > 0 && current.includes(x, z.wrapping_sub(1)) {
            near = true;
        }
        let cells_z = vcall!(dev.city(), 0x178 => fn() -> i32);
        let current = Region::from_ptr(dev.restriction());
        (z < cells_z && current.includes(x, z.wrapping_add(1))) || near
    }
}

port! {
    /// Build custom lots or process the normal zone's connected components, including preview selection,
    /// street passes, zone reconciliation and defer-occupant messages. Forward the incoming argument
    /// frame because custom sizing also uses the caller's region argument word as its orientation list.
    #[port(address = 0x0073_3770, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::OnZone", naked)]
    pub unsafe extern "C" fn fc4p_00733770() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 8", sym on_zone);
    }
}

unsafe extern "fastcall" fn on_zone(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let source = read_unaligned(frame.add(1)) as *mut u8;
        let out = read_unaligned(frame.add(2)) as *mut *mut u8;
        let dev = ZoneDeveloper::from_ptr(this);
        let selection = Selection::from_ptr(this);
        if selection.custom_size() != 0 {
            dev.set_restriction(source);
            examine(this);
            dev.set_restriction(null_mut());
            let mut lot = null_mut();
            let header = empty_list();
            write_unaligned(frame.add(1), header as u32);
            append_orientation(header, dev.orientation_d());
            append_orientation(header, dev.orientation_q());
            append_orientation(header, dev.orientation_d().wrapping_sub(2) & 3);
            append_orientation(header, dev.orientation_q().wrapping_sub(2) & 3);
            let region = Region::from_ptr(source);
            let result = face_road(
                this,
                region.x1(),
                region.z1(),
                region.x2().wrapping_sub(region.x1()).wrapping_add(1),
                region.z2().wrapping_sub(region.z1()).wrapping_add(1),
                frame.add(1).cast(),
                &mut lot,
            );
            if result as u8 == 0 && !out.is_null() {
                write_unaligned(out, new_region(source));
            }
            return destroy_list(frame.add(1).cast());
        }

        defer(dev, 1);
        let header = empty_list();
        let mut list = header;
        dev.set_restriction(parcellizable(this, source));
        compress(this, this.add(0xA4).cast());
        let cropped = dev.restriction();
        lot_size(this, cropped);
        if dev.place_streets() != 0 {
            for kind in [11, 3, 10, 0, 6] {
                extend_streets(this, cropped, kind);
            }
            let region = Region::from_ptr(cropped);
            if region.x2().wrapping_sub(region.x1()).wrapping_add(1) > 2
                && region.z2().wrapping_sub(region.z1()).wrapping_add(1) > 2
            {
                for kind in [11, 3, 10, 0, 6] {
                    jog_streets(this, cropped, kind);
                }
            }
        }
        subdivide(this, dev.restriction(), &mut list);
        delete_region(dev.restriction());
        dev.set_restriction(null_mut());
        if !out.is_null() {
            write_unaligned(out, new_region(source));
            let region = Region::from_ptr(source);
            let width = region.width();
            let height = region.height();
            let mut x = 0i32;
            while x < width {
                let mut z = 0i32;
                while z < height {
                    let output = Region::from_ptr(read_unaligned(out));
                    let value = region.get(x as u32, z as u32);
                    output.set(x as u32, z as u32, value);
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
        }
        let mut node = read_unaligned(header.cast::<*mut u8>());
        while node != header {
            let component = read_unaligned(node.add(8).cast::<*mut u8>());
            dev.set_restriction(component);
            let point = selection.point();
            if point.is_null() || selected(dev, Region::from_ptr(component), point) {
                examine(this);
                match dev.current_zone() as i32 {
                    1..=6 => parcellize(this),
                    7 => simple_parcellize(this, 16),
                    _ => simple_parcellize(this, 6),
                }
            } else if !out.is_null() {
                let component = Region::from_ptr(dev.restriction());
                let width = component.width();
                let height = component.height();
                let mut x = 0i32;
                while x < width {
                    let mut z = 0i32;
                    while z < height {
                        if Region::from_ptr(dev.restriction()).get(x as u32, z as u32) {
                            let output = Region::from_ptr(read_unaligned(out));
                            output.set(
                                component.x1().wrapping_sub(output.x1()).wrapping_add(x) as u32,
                                component.z1().wrapping_sub(output.z1()).wrapping_add(z) as u32,
                                false,
                            );
                        }
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
            }
            delete_component(dev.restriction());
            dev.set_restriction(null_mut());
            node = read_unaligned(node.cast::<*mut u8>());
        }
        clear_nodes(header, 0);
        let point = selection.point();
        if !point.is_null() && !out.is_null() {
            let output = Region::from_ptr(read_unaligned(out));
            let x = read_unaligned(point).wrapping_sub(output.x1()) as u32;
            let z = read_unaligned(point.add(1)).wrapping_sub(output.z1()) as u32;
            let mut result = null_mut();
            if output.get(x, z) {
                result = extract(output.as_ptr(), x, z);
                compress(this, &mut result);
            }
            delete_region(read_unaligned(out));
            write_unaligned(out, result);
        }
        if dev.real_mode() != 0 {
            update_zone(this, source);
        }
        let result = defer(dev, 0);
        let result = clear_nodes(header, result);
        free_node_registers(header, result as u32, (result >> 32) as u32)
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_00733770::PORT];

#[cfg(test)]
mod tests;
