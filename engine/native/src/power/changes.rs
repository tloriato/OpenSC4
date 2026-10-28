// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Wire rebuilds, terrain changes, and neighbor connection selection.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// Services and working bitmaps used while rebuilding a power network.
    pub(super) struct ChangeSim {
        0x18 => city / set_city: *mut u8,
        0x30 => neighbors / set_neighbors: *mut u8,
        0x80 => grid / set_grid: *mut u8,
        0x2C4 => selected / set_selected: *mut u32,
        0x2D0 => next_powered / set_next_powered: *mut u32,
    }
}

game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_reference(this: *mut u32) = "thiscall" 0x0041_1870; }
game_fn! { fn clear_connections(this: *mut u32) = "thiscall" 0x0041_1D70; }
game_fn! { fn pixel_set(bits: *mut u32, width: u32, x: u32, z: u32) = "C" 0x0060_3DA0; }
game_fn! { fn clear_occupants(this: *mut u32) = "thiscall" 0x0041_3E60; }
game_fn! { fn expand(this: *mut u8, rect: *mut Rect) = "thiscall" 0x006E_AE20; }
game_fn! { fn set_wire(this: *mut u8, rect: *mut Rect) = "thiscall" 0x006E_AFD0; }
game_fn! { fn neighborhood(this: *mut u8, x: u32, z: u32, rect: *mut Rect) = "thiscall" 0x006E_AD70; }
game_fn! { fn rect_set(bits: *mut u32, width: u32, rect: *mut Rect) = "C" 0x0060_4180; }
game_fn! { fn pixel_test(bits: *mut u32, width: u32, x: u32, z: u32) -> u32 = "C" 0x0060_3DD0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(p: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn remove_connection(this: *mut u8, other: *mut u8, flag: u32) = "thiscall" 0x0064_DA00; }
game_data! { static connection_allocator: *mut u8 = 0x00B2_07DC; }
game_data! { pub(super) static occupant_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static global_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }

/// Obtain the game's cached list allocator. The temporary reference is released after caching it.
pub(super) unsafe fn allocator(cache: *mut *mut u8, temporary: *mut u32) -> *mut u8 {
    unsafe {
        if cache.read().is_null() {
            temporary.write(0);
            let fw = framework();
            if !fw.is_null() {
                vcall!(fw, 0x14 => fn(u32, u32, *mut u32), 0x3AE4_BEA3, 0x3AE4_BEAB, temporary);
            }
            cache.write(temporary.read() as *mut u8);
            release_reference(temporary);
        }
        cache.read()
    }
}

pub(super) unsafe fn new_head(cache: *mut *mut u8, temporary: *mut u32) -> *mut u8 {
    unsafe {
        let head = vcall!(allocator(cache, temporary), 0x0C => fn(u32) -> *mut u8, 12);
        write_unaligned(head.cast::<*mut u8>(), head);
        write_unaligned(head.add(4).cast::<*mut u8>(), head);
        head
    }
}

pub(super) unsafe fn pointer(p: *mut u8, offset: usize) -> *mut u8 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

/// Match the signed quotient used for bitmap rows; x is shifted as an unsigned word.
pub(super) unsafe fn selected(sim: ChangeSim, x: u32, z: u32) -> bool {
    unsafe {
        let width = vcall!(sim.city(), 0x174 => fn() -> i32);
        let index = (width / 32).wrapping_mul(z as i32).wrapping_add((x >> 5) as i32);
        read_unaligned(sim.selected().wrapping_offset(index as isize)) & 1u32.wrapping_shl(x) != 0
    }
}

port! {
    /// Rebuild conducting footprints around a changed rectangle, then publish its wired/powered cell values.
    /// Reserve the original temporary words so the grid argument retains its original upper three bytes.
    #[port(address = 0x006E_D6E0, owner = "power", name = "cSC4PowerSimulator::UpdateWireBits", naked)]
    pub unsafe extern "C" fn fc4p_006ed6e0() {
        core::arch::naked_asm!(
            "sub esp, 0x7c", "push esi", "push edi", "push ebx", "push ebp",
            "mov edx, esp", "call {body}",
            "pop ebp", "pop ebx", "pop edi", "pop esi", "add esp, 0x7c", "ret 8",
            body = sym update_wire,
        );
    }
}

unsafe extern "fastcall" fn update_wire(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let power = PowerSim::from_ptr(this);
        let sim = ChangeSim::from_ptr(this);
        let list = frame.add(0x10).cast::<u32>();
        list.write(0);
        let head = new_head(occupant_allocator(), frame.add(0x14).cast());
        list.write(head as u32);
        let rect = frame.add(0x90).cast::<*mut Rect>().read();
        let twice_radius = power.radius().wrapping_mul(2);
        // Query bounds are signed and deliberately extend beyond the city.
        frame
            .add(0x48)
            .cast::<u32>()
            .write(read_unaligned(core::ptr::addr_of!((*rect).z0)).wrapping_sub(twice_radius));
        frame
            .add(0x40)
            .cast::<u32>()
            .write(read_unaligned(core::ptr::addr_of!((*rect).z1)).wrapping_add(twice_radius));
        frame
            .add(0x44)
            .cast::<u32>()
            .write(read_unaligned(core::ptr::addr_of!((*rect).x0)).wrapping_sub(twice_radius));
        frame
            .add(0x3C)
            .cast::<u32>()
            .write(read_unaligned(core::ptr::addr_of!((*rect).x1)).wrapping_add(twice_radius));
        let ok = vcall!(*occupant_manager(), 0xA0 => fn(*mut u32, *mut u32, *mut u32, u32, u32) -> u32,
            list, frame.add(0x44).cast(), frame.add(0x3C).cast(), 0, u32::MAX);
        if ok as u8 != 0 {
            let head = list.read() as *mut u8;
            let mut node = pointer(head, 0);
            while node != head {
                let occupant = pointer(node, 8);
                if occupant as u32 != frame.add(0x94).cast::<u32>().read() {
                    let mut relevant = false;
                    for kind in [0x2781_28A0, 0x2890_D4DE, 0x0990_B2EA, 0xC772_BF98] {
                        if vcall!(occupant, 0x1C => fn() -> u32) == kind {
                            relevant = true;
                            break;
                        }
                    }
                    if relevant {
                        let kind = vcall!(occupant, 0x1C => fn() -> u32);
                        if kind == 0xC772_BF98 {
                            let out = frame.add(0x14).cast::<u32>();
                            out.write(0);
                            if vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, 0xA821_EF94, out) as u8 != 0 {
                                let flags = vcall!(out.read() as *mut u8, 0x30 => fn() -> u32);
                                if flags & 0x40000 != 0 {
                                    vcall!(out.read() as *mut u8, 0xEC => fn(*mut u32, *mut u32),
                                        frame.add(0x20).cast(), frame.add(0x24).cast());
                                    let x = frame.add(0x20).cast::<u32>().read();
                                    let z = frame.add(0x24).cast::<u32>().read();
                                    let r = frame.add(0x7C).cast::<Rect>();
                                    r.write_unaligned(Rect {
                                        x0: x,
                                        z0: z,
                                        x1: x,
                                        z1: z,
                                    });
                                    set_wire(this, r);
                                }
                                vcall!(out.read() as *mut u8, 8 => fn());
                            }
                        } else if kind == 0x2890_D4DE || kind == 0x0990_B2EA {
                            let (slot, iid, out_offset, cell_offset, rect_offset) = if kind == 0x2890_D4DE {
                                (0x20, 0xA8C8_6142, 0x18, 0x28, 0x5C)
                            } else {
                                (0x14, 0xC98F_3B24, 0x1C, 0x30, 0x6C)
                            };
                            let out = frame.add(out_offset).cast::<u32>();
                            if vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, iid, out) as u8 != 0 {
                                vcall!(out.read() as *mut u8, slot => fn(*mut u32, *mut u32),
                                    frame.add(cell_offset).cast(), frame.add(cell_offset + 4).cast());
                                let x = frame.add(cell_offset).cast::<u32>().read();
                                let z = frame.add(cell_offset + 4).cast::<u32>().read();
                                let r = frame.add(rect_offset).cast::<Rect>();
                                r.write_unaligned(Rect {
                                    x0: x,
                                    z0: z,
                                    x1: x,
                                    z1: z,
                                });
                                expand(this, r);
                                set_wire(this, r);
                                vcall!(out.read() as *mut u8, 8 => fn());
                                out.write(0);
                            }
                        } else {
                            let r = frame.add(0x4C).cast::<Rect>();
                            vcall!(occupant, 0x2C => fn(*mut Rect), r);
                            expand(this, r);
                            set_wire(this, r);
                        }
                    }
                }
                vcall!(occupant, 8 => fn());
                node = pointer(node, 0);
            }
        }
        let width = vcall!(power.city(), 0x174 => fn() -> u32);
        frame.add(0x1C).cast::<u32>().write(width);
        let mut x = read_unaligned(core::ptr::addr_of!((*rect).x0));
        while x <= read_unaligned(core::ptr::addr_of!((*rect).x1)) {
            let mut z = read_unaligned(core::ptr::addr_of!((*rect).z0));
            while z <= read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                if vcall!(power.zones(), 0x30 => fn(u32, u32) -> u32, x, z) as u8 != 0 {
                    let r = frame.add(0x4C).cast::<Rect>();
                    neighborhood(this, x, z, r);
                    let copy = frame.add(0x7C).cast::<Rect>();
                    copy.write_unaligned(r.read_unaligned());
                    let width = vcall!(power.city(), 0x174 => fn() -> u32);
                    rect_set(power.wire(), width, copy);
                }
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        let mut x = read_unaligned(core::ptr::addr_of!((*rect).x0));
        while x <= read_unaligned(core::ptr::addr_of!((*rect).x1)) {
            let mut z = read_unaligned(core::ptr::addr_of!((*rect).z0));
            while z <= read_unaligned(core::ptr::addr_of!((*rect).z1)) {
                let wired = pixel_test(power.wire(), width, x, z) as u8;
                let powered = pixel_test(power.powered(), width, x, z) as u8;
                frame
                    .add(0x18)
                    .write(if wired == 0 { 0 } else { 1 + u8::from(powered != 0) });
                let value = frame.add(0x18).cast::<u32>().read();
                vcall!(sim.grid(), 0x68 => fn(u32, u32, u32), x, z, value);
                z = z.wrapping_add(1);
            }
            x = x.wrapping_add(1);
        }
        clear_occupants(list);
        let saved = list.read();
        let alloc = allocator(occupant_allocator(), frame.add(0x1C).cast());
        vcall!(alloc, 0x10 => fn(u32) -> u64, saved)
    }
}

port! {
    /// Find the first connection to a city in the selected network, optionally mark it visited and powered.
    /// The connection's z coordinate overwrites the caller's city argument word. Only flag low bytes count.
    #[port(address = 0x006E_DB40, owner = "power", name = "FindSelectedPowerConnection", naked)]
    pub unsafe extern "C" fn fc4p_006edb40() {
        core::arch::naked_asm!(
            "sub esp, 0xc", "push ebx", "push ebp", "push esi", "push edi",
            "mov edx, esp", "call {body}",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0xc", "ret 12",
            body = sym find_connection,
        );
    }
}

unsafe extern "fastcall" fn find_connection(this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let sim = ChangeSim::from_ptr(this);
        let head_word = frame.add(0x10).cast::<u32>();
        head_word.write(0);
        let head = new_head(connection_allocator(), frame.add(0x18).cast());
        head_word.write(head as u32);
        vcall!(sim.neighbors(), 0x48 => fn(u32, u32, *mut u32), 4,
            frame.add(0x20).cast::<u32>().read(), head_word);
        let head = head_word.read() as *mut u8;
        let mut node = pointer(head, 0);
        let mut found = false;
        while node != head {
            frame.add(0x18).cast::<u32>().write(node as u32);
            let connection = pointer(node, 8);
            if !connection.is_null() {
                let xmax = vcall!(*global_city(), 0x174 => fn() -> i32).wrapping_sub(1);
                let zmax = vcall!(*global_city(), 0x178 => fn() -> i32).wrapping_sub(1);
                vcall!(connection, 0x18 => fn(*mut i32, *mut i32),
                    frame.add(0x14).cast(), frame.add(0x20).cast());
                let x = frame.add(0x14).cast::<i32>();
                let z = frame.add(0x20).cast::<i32>();
                if x.read() < 0 {
                    x.write(0);
                }
                if z.read() < 0 {
                    z.write(0);
                }
                if x.read() > xmax {
                    x.write(xmax);
                }
                if z.read() > zmax {
                    z.write(zmax);
                }
            }
            let x = frame.add(0x14).cast::<u32>().read();
            let z = frame.add(0x20).cast::<u32>().read();
            if selected(sim, x, z) {
                if frame.add(0x24).read() != 0 {
                    vcall!(pointer(frame.add(0x18).cast::<u32>().read() as *mut u8, 8),
                        0x30 => fn(u32), 1);
                }
                if frame.add(0x28).read() != 0 {
                    let width = vcall!(sim.city(), 0x174 => fn() -> u32);
                    pixel_set(
                        sim.next_powered(),
                        width,
                        frame.add(0x14).cast::<u32>().read(),
                        frame.add(0x20).cast::<u32>().read(),
                    );
                }
                found = true;
                break;
            }
            node = pointer(frame.add(0x18).cast::<u32>().read() as *mut u8, 0);
        }
        clear_connections(head_word);
        let saved_head = head_word.read();
        let alloc = allocator(connection_allocator(), frame.add(0x24).cast());
        let result = vcall!(alloc, 0x10 => fn(u32) -> u64, saved_head);
        (result & !255) | u64::from(found)
    }
}

port! {
    /// Detach poles inside the changed bounds, then remove one connection per ordered pole pair.
    /// Interface outputs and the duplicate flag use the original caller argument words as scratch storage.
    #[port(address = 0x006E_DDA0, owner = "power", name = "cSC4PowerSimulator::DoTerrainHeightsChanged", naked)]
    pub unsafe extern "C" fn fc4p_006edda0() {
        core::arch::naked_asm!(
            "sub esp, 0x24", "push ebx", "push ebp", "push esi", "push edi",
            "mov edx, esp", "call {body}",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x24", "ret 8",
            body = sym terrain_changed,
        );
    }
}

unsafe extern "fastcall" fn terrain_changed(_this: *mut u8, frame: *mut u8) -> u64 {
    unsafe {
        let manager = *occupant_manager();
        frame.add(0x18).cast::<u32>().write(manager as u32);
        if !manager.is_null() {
            vcall!(manager, 4 => fn());
        }
        let list = frame.add(0x14).cast::<u32>();
        list.write(0);
        let head = new_head(occupant_allocator(), frame.add(0x1C).cast());
        list.write(head as u32);
        let low = frame.add(0x38).cast::<*mut i32>().read();
        let high = frame.add(0x3C).cast::<*mut i32>().read();
        let ok = vcall!(manager, 0xA0 => fn(*mut u32, *mut i32, *mut i32, u32, u32) -> u32,
            list, low, high, 0, u32::MAX);
        if ok as u8 != 0 {
            let head = list.read() as *mut u8;
            let mut node = pointer(head, 0);
            while node != head {
                let occupant = pointer(node, 8);
                let pole_out = frame.add(0x38).cast::<u32>();
                if vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, 0xA8C8_6142, pole_out) as u8 != 0 {
                    vcall!(pole_out.read() as *mut u8, 0x20 => fn(*mut i32, *mut i32),
                        frame.add(0x2C).cast(), frame.add(0x28).cast());
                    let x = frame.add(0x2C).cast::<i32>().read();
                    let z = frame.add(0x28).cast::<i32>().read();
                    let high = frame.add(0x3C).cast::<*mut i32>().read();
                    let inside = x >= low.read_unaligned()
                        && x <= high.read_unaligned()
                        && z >= low.add(1).read_unaligned()
                        && z <= high.add(1).read_unaligned();
                    if inside {
                        vcall!(pole_out.read() as *mut u8, 0x18 => fn(u32), 1);
                    }
                    vcall!(pole_out.read() as *mut u8, 8 => fn());
                    if inside {
                        vcall!(frame.add(0x18).cast::<*mut u8>().read(), 0x54 => fn(*mut u8, u32, u32), occupant, 1, 0);
                    }
                }
                node = pointer(node, 0);
            }
        }
        let unique = allocate(12);
        unique.cast::<*mut u8>().write_unaligned(unique);
        unique.add(4).cast::<*mut u8>().write_unaligned(unique);
        let head = list.read() as *mut u8;
        let mut node = pointer(head, 0);
        while node != head {
            let occupant = pointer(node, 8);
            let line_out = frame.add(0x28).cast::<u32>();
            if vcall!(occupant, 0 => fn(u32, *mut u32) -> u32, 0xC98F_3B24, line_out) as u8 != 0 {
                vcall!(line_out.read() as *mut u8, 0x0C => fn(*mut u32, *mut u32, *mut u8),
                    frame.add(0x20).cast(), frame.add(0x24).cast(), frame.add(0x38));
                frame.add(0x3C).write(0);
                if frame.add(0x20).cast::<u32>().read() != 0 && frame.add(0x24).cast::<u32>().read() != 0 {
                    let mut prior = pointer(unique, 0);
                    while prior != unique {
                        vcall!(pointer(prior, 8), 0x0C => fn(*mut u32, *mut u32, *mut u8),
                            frame.add(0x1C).cast(), frame.add(0x2C).cast(), frame.add(0x13));
                        let p = frame.add(0x1C).cast::<*mut u8>().read();
                        if !p.is_null() {
                            vcall!(p, 8 => fn());
                        }
                        let p = frame.add(0x2C).cast::<*mut u8>().read();
                        if !p.is_null() {
                            vcall!(p, 8 => fn());
                        }
                        let a = frame.add(0x1C).cast::<u32>().read();
                        let b = frame.add(0x2C).cast::<u32>().read();
                        if a != 0
                            && b != 0
                            && a == frame.add(0x20).cast::<u32>().read()
                            && b == frame.add(0x24).cast::<u32>().read()
                        {
                            frame.add(0x3C).write(1);
                            break;
                        }
                        prior = pointer(prior, 0);
                    }
                    if frame.add(0x3C).read() == 0 {
                        let n = allocate(12);
                        let value = n.wrapping_add(8);
                        if !value.is_null() {
                            value.cast::<u32>().write_unaligned(line_out.read());
                        }
                        let last = pointer(unique, 4);
                        n.cast::<*mut u8>().write_unaligned(unique);
                        n.add(4).cast::<*mut u8>().write_unaligned(last);
                        last.cast::<*mut u8>().write_unaligned(n);
                        unique.add(4).cast::<*mut u8>().write_unaligned(n);
                    }
                }
                let p = frame.add(0x20).cast::<*mut u8>().read();
                if !p.is_null() {
                    vcall!(p, 8 => fn());
                }
                let p = frame.add(0x24).cast::<*mut u8>().read();
                if !p.is_null() {
                    vcall!(p, 8 => fn());
                }
                if frame.add(0x3C).read() != 0 {
                    vcall!(line_out.read() as *mut u8, 8 => fn());
                }
            }
            vcall!(occupant, 8 => fn());
            node = pointer(node, 0);
        }
        let mut node = pointer(unique, 0);
        while node != unique {
            vcall!(pointer(node, 8), 0x0C => fn(*mut u32, *mut u32, *mut u8),
                frame.add(0x38).cast(), frame.add(0x2C).cast(), frame.add(0x3C));
            vcall!(pointer(node, 8), 8 => fn());
            remove_connection(
                frame.add(0x38).cast::<*mut u8>().read(),
                frame.add(0x2C).cast::<*mut u8>().read(),
                0,
            );
            let p = frame.add(0x38).cast::<*mut u8>().read();
            if !p.is_null() {
                vcall!(p, 8 => fn());
            }
            let p = frame.add(0x2C).cast::<*mut u8>().read();
            if !p.is_null() {
                vcall!(p, 8 => fn());
            }
            node = pointer(node, 0);
        }
        let mut node = pointer(unique, 0);
        while node != unique {
            let next = pointer(node, 0);
            free(node);
            node = next;
        }
        unique.cast::<*mut u8>().write_unaligned(unique);
        unique.add(4).cast::<*mut u8>().write_unaligned(unique);
        free(unique);
        clear_occupants(list);
        let saved_head = list.read();
        let alloc = allocator(occupant_allocator(), frame.add(0x3C).cast());
        let result = vcall!(alloc, 0x10 => fn(u32) -> u64, saved_head);
        // The final release receives EAX from the head deallocation and EDX from the manager's table.
        let mut call = crate::abi::RawCall::new(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(manager, 8),
            manager as u32,
            &[],
        );
        call.eax = result as u32;
        call.edx = manager.cast::<u32>().read_unaligned();
        crate::abi::call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}

pub const PORTS: &[Port] = &[fc4p_006ed6e0::PORT, fc4p_006edb40::PORT, fc4p_006edda0::PORT];

#[cfg(test)]
mod tests;
