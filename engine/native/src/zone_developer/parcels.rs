// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Register newly constructed lots and split the buildable region into connected components.

use super::{manager::LotManager, network::virtual_call, region::Region, Port, ZoneDeveloper};
use core::ptr::{null_mut, read_unaligned, write_unaligned};

layout! {
    /// Pending lots, relative to the lot-manager interface.
    struct PendingLots {
        0x5C => header / set_header: *mut u8,
        0x64 => count / set_count: u32,
    }
}

game_fn! { fn allocate_lot(size: u32) -> *mut u8 = "C" 0x006B_F430; }
game_fn! { fn construct_lot(this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32) -> *mut u8 = "thiscall" 0x006C_1E50; }
game_fn! { fn insert_pending(this: *mut u8, lot: *mut *mut u8) = "thiscall" 0x0072_C810; }
game_fn! { fn allocate_node(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_node(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn draw_street(this: *mut u8, x1: i32, z1: i32, x2: i32, z2: i32, per_cell: u32) = "thiscall" 0x0072_A2A0; }
game_fn! { #[allow(clippy::too_many_arguments)] fn face_road(this: *mut u8, x: i32, z: i32, width: i32, height: i32, list: *mut *mut u8, out: *mut *mut u8) = "thiscall" 0x0072_D200; }
game_fn! { fn free(memory: *mut u8) = "C" 0x005E_5620; }
game_fn! { fn buildable(this: *mut u8, region: *mut u8) -> *mut u8 = "thiscall" 0x0072_CB90; }
game_fn! { fn extract_component(this: *mut u8, region: *mut u8, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0072_CDA0; }
game_fn! { fn compress(this: *mut u8, region: *mut *mut u8) = "thiscall" 0x0072_AF30; }
game_data! { static developer_service: *mut u8 = 0x00B4_3DA8; }
game_data! { static city: *mut u8 = 0x00B4_3D68; }

/// Append to the cached sentinel without touching its unused payload or the list owner's other fields.
unsafe fn append(header: *mut u8, slot: *const *mut u8) {
    unsafe {
        let node = allocate_node(12);
        let payload = node.wrapping_add(8).cast::<*mut u8>();
        if !payload.is_null() {
            write_unaligned(payload, read_unaligned(slot));
        }
        let previous = read_unaligned(header.add(4).cast::<*mut u8>());
        write_unaligned(node.cast::<*mut u8>(), header);
        write_unaligned(node.add(4).cast::<*mut u8>(), previous);
        write_unaligned(previous.cast::<*mut u8>(), node);
        write_unaligned(header.add(4).cast::<*mut u8>(), node);
    }
}

/// Free a region's columns, column array and shell in that order, rereading the width after each free.
unsafe fn delete_region(region: *mut u8) {
    unsafe {
        if region.is_null() {
            return;
        }
        let region = Region::from_ptr(region);
        region.set_vtable(0x00A8_6364);
        if !region.columns().is_null() {
            let mut x = 0u32;
            while x < region.width() as u32 {
                free(read_unaligned(region.columns().wrapping_add(x as usize)).cast());
                x = x.wrapping_add(1);
            }
            free(region.columns().cast());
        }
        free(region.as_ptr());
    }
}

port! {
    /// Construct and initialize a lot, retain its map reference, and fill its rectangle in Z-major order.
    /// Nonpositive dimensions still cover the anchor cell. Register it with the pending-lot iterator or
    /// local list, then the occupant manager. EDX retains the last occupant call's value.
    #[port(address = 0x0072_E520, owner = "zonedeveloper", name = "CreateAndRegisterLot")]
    pub unsafe extern "thiscall" fn fc4p_0072e520(
        this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32, out: *mut *mut u8
    ) -> u64 {
        unsafe {
            let memory = allocate_lot(0x8C);
            let lot = if memory.is_null() { null_mut() } else { construct_lot(memory, x, z, width, height, facing) };
            write_unaligned(out, lot);
            vcall!(lot, 4 => fn());
            vcall!(read_unaligned(out), 0x14 => fn());
            vcall!(read_unaligned(out), 0x88 => fn(u32, u32, u32), 1, 0, 0);
            let end_x = x.wrapping_add(width).wrapping_sub(1).max(x);
            let end_z = z.wrapping_add(height).wrapping_sub(1).max(z);
            vcall!(read_unaligned(out), 4 => fn());
            let manager = LotManager::from_ptr(this);
            let mut cz = z;
            loop {
                let mut cx = x;
                loop {
                    let row = read_unaligned(manager.rows().wrapping_offset(cx.wrapping_mul(12) as isize).cast::<*mut u8>());
                    write_unaligned(row.wrapping_offset(cz.wrapping_mul(4) as isize).cast::<*mut u8>(), read_unaligned(out));
                    cx = cx.wrapping_add(1);
                    if cx > end_x { break; }
                }
                cz = cz.wrapping_add(1);
                if cz > end_z { break; }
            }
            let iterator = vcall!(read_unaligned(developer_service()), 0x58 => fn() -> *mut u8);
            if !iterator.is_null() { insert_pending(iterator, out); }
            else {
                let pending = PendingLots::from_ptr(this);
                append(pending.header(), out);
                pending.set_count(pending.count().wrapping_add(1));
            }
            let occupants = vcall!(read_unaligned(city()), 0x90 => fn() -> *mut u8);
            let result = virtual_call(occupants, 0xAC, occupants as u32,
                read_unaligned(occupants.cast::<u32>()), &[read_unaligned(out) as u32]);
            (u64::from(result.edx) << 32) | u64::from(!read_unaligned(out).is_null())
        }
    }
}

port! {
    /// Filter buildable cells, scan X before Z, extract each connected component and compress it before
    /// appending it. If the output list is still empty, transfer the compressed filtered region instead.
    /// Otherwise free the filtered region. The restriction's dimensions determine the scan extent.
    #[port(address = 0x0072_EC60, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::SubdivideTheZone")]
    pub unsafe extern "thiscall" fn fc4p_0072ec60(this: *mut u8, source: *mut u8, out: *mut *mut u8) {
        unsafe {
            let dev = ZoneDeveloper::from_ptr(this);
            let mut filtered = buildable(this, source);
            let restriction = Region::from_ptr(dev.restriction());
            let (width, height) = (restriction.width(), restriction.height());
            let region = Region::from_ptr(filtered);
            let mut x = 0i32;
            while x < width {
                let mut z = 0i32;
                while z < height {
                    if region.get(x as u32, z as u32) {
                        let mut component = extract_component(this, filtered, x, z);
                        compress(this, &mut component);
                        append(read_unaligned(out), &component);
                    }
                    z = z.wrapping_add(1);
                }
                x = x.wrapping_add(1);
            }
            let header = read_unaligned(out);
            if read_unaligned(header.cast::<*mut u8>()) == header {
                compress(this, &mut filtered);
                append(read_unaligned(out), &filtered);
            } else { delete_region(filtered); }
        }
    }
}

port! {
    /// Place optional street lines using Q's start-side table, then try a one-cell lot at every coordinate,
    /// X before Z, with a fresh empty orientation list each time. Release returned lots and list nodes.
    /// Delete and clear the current restriction after placement. For an invalid Q the start coordinates
    /// are the spacing argument, and the direction bytes retain their incoming stack contents.
    #[port(address = 0x0072_E9B0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::SimpleParcellize", naked)]
    pub unsafe extern "C" fn fc4p_0072e9b0() {
        core::arch::naked_asm!("movzx edx, word ptr [esp - 14]", "jmp {body}", body = sym simple_parcellize);
    }
}

unsafe extern "fastcall" fn simple_parcellize(this: *mut u8, incoming_directions: u32, spacing: i32) {
    unsafe {
        let dev = ZoneDeveloper::from_ptr(this);
        let region = Region::from_ptr(dev.restriction());
        let q = dev.orientation_q();
        let d = dev.orientation_d();
        let (start_x, start_z, forward_x, forward_z) = match q {
            0 | 2 => (
                if d == 3 { region.x1() } else { region.x2() },
                if q == 0 { region.z1() } else { region.z2() },
                d == 3,
                q == 0,
            ),
            1 => (
                region.x2(),
                if d == 0 { region.z1() } else { region.z2() },
                false,
                d == 0,
            ),
            3 => (
                region.x1(),
                if d == 0 { region.z1() } else { region.z2() },
                true,
                d == 0,
            ),
            _ => (
                spacing,
                spacing,
                incoming_directions as u8 != 0,
                (incoming_directions >> 8) as u8 != 0,
            ),
        };
        if dev.place_streets() != 0 {
            let mut x = start_x;
            let mut count = 0i32;
            while x >= region.x1() && x <= region.x2() {
                if count >= spacing && x.wrapping_sub(region.x1()) >= 2 && region.x2().wrapping_sub(x) >= 2 {
                    draw_street(this, x, region.z1(), x, region.z2(), 0);
                    count = 0;
                } else {
                    count = count.wrapping_add(1);
                }
                x = x.wrapping_add(if forward_x { 1 } else { -1 });
            }
            let mut z = start_z;
            count = 0;
            while z >= region.z1() && z <= region.z2() {
                if count >= spacing && z.wrapping_sub(region.z1()) >= 2 && region.z2().wrapping_sub(z) >= 2 {
                    draw_street(this, region.x1(), z, region.x2(), z, 0);
                    count = 0;
                } else {
                    count = count.wrapping_add(1);
                }
                z = z.wrapping_add(if forward_z { 1 } else { -1 });
            }
        }
        let mut x = region.x1();
        while x <= region.x2() {
            let mut z = region.z1();
            while z <= region.z2() {
                let mut lot = null_mut();
                let header = allocate_node(12);
                write_unaligned(header.cast::<*mut u8>(), header);
                write_unaligned(header.add(4).cast::<*mut u8>(), header);
                let mut list = header;
                face_road(this, x, z, 1, 1, &mut list, &mut lot);
                if !lot.is_null() {
                    vcall!(lot, 8 => fn());
                }
                let mut node = read_unaligned(header.cast::<*mut u8>());
                while node != header {
                    let old = node;
                    node = read_unaligned(node.cast::<*mut u8>());
                    free_node(old);
                }
                write_unaligned(header.cast::<*mut u8>(), header);
                write_unaligned(header.add(4).cast::<*mut u8>(), header);
                free_node(header);
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        delete_region(dev.restriction());
        dev.set_restriction(null_mut());
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072e520::PORT, fc4p_0072e9b0::PORT, fc4p_0072ec60::PORT];

#[cfg(test)]
mod tests;
