// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Time-sliced lot and neighbor population/capacity accumulation.

use super::late_containers::call;
use super::*;
use crate::abi::{Conv, RawCall};

layout! {
    /// The working subnetwork records and the city-edge coordinate mappings.
    pub struct InfoSim {
        0x20 => edge_x / set_edge_x: i32,
        0x24 => edge_z / set_edge_z: i32,
        0x28 => mapped_x / set_mapped_x: i32,
        0x2C => mapped_z / set_mapped_z: i32,
        0x64 => working / set_working: *mut u8,
        0x7C => rows / set_rows: *mut u8,
        0x194 => max_x / set_max_x: i32,
        0x198 => max_z / set_max_z: i32,
    }
}

game_fn! { fn allocate(bytes: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn copy(destination: *mut u8, source: *const u8, bytes: u32) -> *mut u8 = "C" 0x009E_EB70; }
game_fn! { #[allow(dead_code)] fn expired(this: *mut u8) -> u32 = "thiscall" 0x0068_BF80; }
game_fn! { fn next_boundary(this: *mut u8, x: *mut i32, z: *mut i32) -> u8 = "thiscall" 0x0060_CEE0; }
game_fn! { fn set_property(holder: *mut u8, id: u32, vector: *const u32) = "C" 0x005F_E6B0; }
game_fn! { fn population(this: *mut u8, key: *const u32) -> *mut u32 = "thiscall" 0x0049_7A00; }
game_fn! { fn neighbor_holder(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0071_14E0; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_holder(this: *mut u32) = "thiscall" 0x0041_1870; }
game_fn! { fn clear_list(this: *mut u32) = "thiscall" 0x0041_1D70; }
game_data! { static initialized: u32 = 0x00B4_B344; }
game_data! { static stride: u32 = 0x00B4_B33C; }
game_data! { static balance: i32 = 0x00B4_B340; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }
game_data! { static network_manager: *mut u8 = 0x00B4_3D8C; }
game_data! { static list_allocator: *mut u8 = 0x00B2_07DC; }

unsafe fn find_car(sim: InfoSim, x: i32, z: i32) -> *mut u8 {
    unsafe {
        let row = word(sim.rows(), x.wrapping_mul(12) as usize) as *mut u8;
        let mut p = word(row, z.wrapping_mul(4) as usize) as *mut u8;
        while !p.is_null() && p.add(6).read() != 1 {
            p = word(p, 20) as *mut u8;
        }
        p
    }
}

unsafe fn next_tree(mut p: *mut u8) -> *mut u8 {
    unsafe {
        let mut q = word(p, 12) as *mut u8;
        if !q.is_null() {
            p = q;
            q = word(p, 8) as *mut u8;
            while !q.is_null() {
                p = q;
                q = word(p, 8) as *mut u8;
            }
        } else {
            q = word(p, 4) as *mut u8;
            while p == word(q, 12) as *mut u8 {
                p = q;
                q = word(q, 4) as *mut u8;
            }
            if word(p, 12) as *mut u8 != q {
                p = q;
            }
        }
        p
    }
}

unsafe fn append(vector: &mut [u32; 3], value: u32) {
    unsafe {
        let mut end = vector[1] as *mut u32;
        if vector[1] == vector[2] {
            let len = (vector[1].wrapping_sub(vector[0]) as i32 >> 2) as u32;
            let count = len.wrapping_add(len.max(1));
            let fresh = if count == 0 {
                core::ptr::null_mut()
            } else {
                allocate(count.wrapping_mul(4))
            };
            let old = vector[0] as *mut u8;
            end = fresh.cast();
            if vector[1] != vector[0] {
                let bytes = vector[1].wrapping_sub(vector[0]);
                end = copy(fresh, old, bytes).wrapping_add(bytes as usize).cast();
            }
            end.write(value);
            let mut r = Registers {
                eax: vector[0],
                ecx: value,
                edx: 0,
            };
            if !old.is_null() {
                call(0x0090_CF63, Conv::Cdecl, &[old as u32], &mut r);
            }
            vector[0] = fresh as u32;
            vector[2] = (fresh as u32).wrapping_add(count.wrapping_mul(4));
        } else if !end.is_null() {
            end.write(value);
        }
        vector[1] = end.wrapping_add(1) as u32;
    }
}

unsafe fn allocator() -> *mut u8 {
    unsafe {
        if list_allocator().read().is_null() {
            let mut holder = 0u32;
            let app = framework();
            if !app.is_null() {
                vcall!(app, 0x14 => fn(u32, u32, *mut u32), 0x3AE4_BEA3, 0x3AE4_BEAB, &mut holder);
            }
            list_allocator().write(holder as *mut u8);
            release_holder(&mut holder);
        }
        list_allocator().read()
    }
}

unsafe fn neighbor_capacities(this: *mut u8, sim: InfoSim) -> Registers {
    unsafe {
        let alloc = allocator();
        let mut header = vcall!(alloc, 0x0C => fn(u32) -> u32, 12);
        let head = header as *mut u8;
        store(head, 0, header);
        store(head, 4, header);
        vcall!(*network_manager(), 0x40 => fn(*mut u32), &mut header);
        let head = header as *mut u8;
        let mut node = word(head, 0) as *mut u8;
        while node != head {
            let neighbor = word(node, 8) as *mut u8;
            let mut x = 0i32;
            let mut z = 0i32;
            vcall!(neighbor, 0x18 => fn(*mut i32, *mut i32), &mut x, &mut z);
            let (mut cx, mut cz) = (x, z);
            if x == -1 {
                cx = 0;
            } else if x == sim.edge_x() {
                cx = sim.mapped_x();
            } else if z == -1 {
                cz = 0;
            } else if z == sim.edge_z() {
                cz = sim.mapped_z();
            }
            let info = find_car(sim, cx, cz);
            if !info.is_null() {
                let holder = neighbor_holder(this, 1, x, z);
                if !holder.is_null() {
                    let mut capacity = [0u32; 4];
                    for category in 0..4 {
                        capacity[category as usize] = vcall!(this, 0x50 => fn(*mut u8, u32) -> u32, holder, category);
                    }
                    for side in 0..4 {
                        let id = info.add(12 + side * 2).cast::<u16>().read_unaligned();
                        if id != 0 {
                            let dest = sim.working().wrapping_add(usize::from(id).wrapping_mul(28)).add(12);
                            for (i, c) in capacity.iter().enumerate() {
                                store(dest, i * 4, word(dest, i * 4).wrapping_add(*c));
                            }
                        }
                    }
                }
            }
            node = word(node, 0) as *mut u8;
        }
        clear_list(&mut header);
        let alloc = allocator();
        let args = [header];
        let mut c = RawCall::new(Conv::Thiscall, crate::abi::vslot(alloc, 0x10), alloc as u32, &args);
        c.eax = word(alloc, 0);
        super::network::registers(c)
    }
}

unsafe fn accumulate_lot(this: *mut u8, sim: InfoSim, lot: *mut u8, vector: &mut [u32; 3]) {
    unsafe {
        let mut rect = [0i32; 4];
        vcall!(lot, 0x38 => fn(*mut i32), rect.as_mut_ptr());
        let x0 = rect[0].wrapping_sub(1).max(0);
        let z0 = rect[1].wrapping_sub(1).max(0);
        let x1 = rect[2].wrapping_add(1).min(sim.max_x()).max(x0);
        let z1 = rect[3].wrapping_add(1).min(sim.max_z()).max(z0);
        let mut boundary = [0, x0, z0, 1, x0, z0, x1, z1];
        let mut x = 0i32;
        let mut z = 0i32;
        vector[1] = vector[0];
        let mut count = 0i32;
        while next_boundary(boundary.as_mut_ptr().cast(), &mut x, &mut z) != 0 {
            if (x >= rect[0] && x <= rect[2]) || (z >= rect[1] && z <= rect[3]) {
                let info = find_car(sim, x, z);
                if !info.is_null() {
                    for side in 0..4 {
                        let id = u32::from(info.add(12 + side * 2).cast::<u16>().read_unaligned());
                        if id != 0 {
                            let mut exists = false;
                            for i in 0..count {
                                if (vector[0] as *mut u32).wrapping_offset(i as isize).read() == id {
                                    exists = true;
                                    break;
                                }
                            }
                            if !exists {
                                append(vector, id);
                                count = count.wrapping_add(1);
                            }
                        }
                    }
                }
            }
        }
        if count == 0 {
            let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
            vcall!(holder, 0x44 => fn(u32, u32), 0x2A4F_9C74, 0);
            return;
        }
        let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
        set_property(holder, 0x2A4F_9C74, vector.as_ptr());
        let populations = vcall!(lot, 0xD8 => fn() -> *mut u8);
        let mut row = word(populations, 0) as *mut u8;
        let end = word(populations, 4) as *mut u8;
        while row != end {
            let header = word(row, 0) as *mut u8;
            let mut node = word(header, 8) as *mut u8;
            while node != header {
                let key = word(node, 16);
                let amount = u32::from(node.add(20).cast::<u16>().read_unaligned());
                for i in 0..count {
                    let id = (vector[0] as *mut u32).wrapping_offset(i as isize).read();
                    let dest = sim.working().wrapping_add(id.wrapping_mul(28) as usize);
                    let p = population(dest, &key);
                    p.write(p.read().wrapping_add(amount));
                }
                node = next_tree(node);
            }
            row = row.wrapping_add(12);
        }
        let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
        for i in 0..count {
            let id = (vector[0] as *mut u32).wrapping_offset(i as isize).read();
            let dest = sim.working().wrapping_add(id.wrapping_mul(28) as usize).add(12);
            for category in 0..4 {
                let value = vcall!(this, 0x50 => fn(*mut u8, u32) -> u32, holder, category);
                let offset = category as usize * 4;
                store(dest, offset, word(dest, offset).wrapping_add(value));
            }
        }
    }
}

super::network::register_port!(
    0x0071_6180,
    fc4p_00716180,
    "cSC4TrafficSimulator::UpdateSubnetworkInfo",
    update,
    8,
    [12, 8]
);

unsafe extern "fastcall" fn update(this: *mut u8, out: *mut Registers, time: *const u32, scan: *mut u8) {
    // SAFETY: the original entry supplies the simulator, timer words, and writable scanning iterator.
    unsafe {
        let sim = InfoSim::from_ptr(this);
        if initialized().read() as u8 & 1 == 0 {
            initialized().write(initialized().read() | 1);
            stride().write(1024);
            balance().write(0);
        }
        let lots = lot_manager().read();
        let mut budget = [time.read(), time.add(1).read(), 0, stride().read(), 0, stride() as u32];
        let first = allocate(256) as u32;
        let mut vector = [first, first, first.wrapping_add(256)];
        let mut r = Registers::default();
        let complete = loop {
            budget[2] = budget[2].wrapping_add(budget[3]);
            if budget[2] & 1023 == 0 {
                r.ecx = budget.as_mut_ptr() as u32;
                call(0x0068_BF80, Conv::Thiscall, &[], &mut r);
                if r.eax as u8 != 0 {
                    break false;
                }
            }
            if scan.read() == 2 {
                r = neighbor_capacities(this, sim);
                break true;
            }
            if scan.read() == 0 {
                scan.write(1);
            } else {
                let step = word(scan, 12);
                let x = word(scan, 4).wrapping_add(step);
                store(scan, 4, x);
                if x as i32 > word(scan, 24) as i32 {
                    let z = word(scan, 8).wrapping_add(step);
                    store(scan, 4, word(scan, 16));
                    store(scan, 8, z);
                    if z as i32 > word(scan, 28) as i32 {
                        scan.write(2);
                        r = neighbor_capacities(this, sim);
                        break true;
                    }
                }
            }
            let lot = vcall!(lots, 0x18 => fn(u32, u32, u32) -> *mut u8, word(scan, 4), word(scan, 8), 1);
            if !lot.is_null() {
                accumulate_lot(this, sim, lot, &mut vector);
            }
        };
        r.eax = vector[0];
        if vector[0] != 0 {
            call(0x0090_CF63, Conv::Cdecl, &[vector[0]], &mut r);
        }
        let cycles = budget[2] >> 10;
        r.eax = cycles;
        if cycles != 0 {
            if cycles as i32 > 20 {
                r.eax = balance().read().wrapping_add(1) as u32;
                balance().write(r.eax as i32);
                if r.eax as i32 >= 16 {
                    r.eax = stride().read();
                    balance().write(0);
                    if r.eax > 1 {
                        r.eax >>= 1;
                        stride().write(r.eax);
                    }
                }
            } else if (cycles as i32) < 5 {
                r.eax = balance().read().wrapping_sub(1) as u32;
                balance().write(r.eax as i32);
                if r.eax as i32 <= -4 {
                    r.eax = stride().read();
                    balance().write(0);
                    if r.eax < 1024 {
                        let doubled = r.eax.wrapping_mul(2);
                        stride().write(doubled);
                        if complete {
                            r.ecx = doubled;
                        } else {
                            r.edx = doubled;
                        }
                    }
                }
            }
        }
        r.eax = (r.eax & !255) | u32::from(complete);
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[fc4p_00716180::PORT];

#[cfg(test)]
mod tests;
