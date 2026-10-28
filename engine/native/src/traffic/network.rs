// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network records, boundary connections and density queries.

use super::*;
use crate::abi::{Conv, RawCall};

// A small ABI adapter returns the three integer registers from a Rust body. Its local buffer is dead
// stack storage; the original stack arguments and callee pop are retained.
macro_rules! register_port {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal, [$($offset:literal),*]) => {
        port! {
            #[port(address = $address, owner = "traffic", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push ebp", "mov ebp, esp", "sub esp, 12", "lea edx, [esp]",
                    $(concat!("push dword ptr [ebp + ", stringify!($offset), "]"),)*
                    "call {body}", "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]",
                    "mov edx, [ebp - 4]", "mov esp, ebp", "pop ebp", concat!("ret ", stringify!($pop)),
                    body = sym $body,
                );
            }
        }
    };
}
pub(super) use register_port;

/// Captures ECX as well as EDX:EAX across a game call.
pub(super) unsafe fn registers(mut call: RawCall) -> Registers {
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") &mut call, clobber_abi("C"),
        );
    }
    Registers { eax: call.eax, ecx: call.ecx, edx: call.edx }
}

layout! {
    /// Network fields relative to the traffic simulator's primary interface.
    pub struct NetworkSim {
        0x58 => connections / set_connections: *mut u8,
        0x60 => next_subnetwork / set_next_subnetwork: u16,
        0x70 => subnetworks / set_subnetworks: *mut u8,
        0x74 => subnetworks_end / set_subnetworks_end: *mut u8,
        0x7C => rows / set_rows: *mut u8,
        0xB8 => pool_header / set_pool_header: *mut u8,
        0xC4 => default_pool / set_default_pool: *mut u8,
        0x17C => buckets / set_buckets: *mut u8,
        0x180 => buckets_end / set_buckets_end: *mut u8,
    }
}
layout! {
    /// A pooled, singly linked network-info record. Bytes outside these fields are retained.
    pub struct NetworkInfo {
        0x04 => flags / set_flags: u8,
        0x06 => kind / set_kind: u8,
        0x07 => connection_count / set_connection_count: u8,
        0x08 => inverse_capacity / set_inverse_capacity: u32,
        0x0C => morning_subnetwork / set_morning_subnetwork: u32,
        0x10 => evening_subnetwork / set_evening_subnetwork: u32,
        0x14 => next / set_next: *mut u8,
    }
}

game_fn! { fn inner_cell(this: *mut u8, x: i32, z: i32, ix: *mut i32, iz: *mut i32) -> i32 = "thiscall" 0x0070_DDD0; }
game_fn! { fn allocate_pool(this: *mut u8) -> *mut u8 = "thiscall" 0x0090_CF9A; }
game_fn! { fn delete_info(this: *mut u8, flags: u32) -> u32 = "thiscall" 0x0070_F040; }
game_fn! { fn edge_record(this: *mut u8, x: i32, z: i32) -> *mut u8 = "thiscall" 0x0070_F970; }
game_fn! { #[allow(dead_code)] fn find_pool(this: *mut u8, id: *mut u16) -> *mut u8 = "thiscall" 0x0070_EFC0; }
game_fn! { #[allow(dead_code)] fn property_vector(holder: *mut u8, id: u32, vector: *mut u32) -> u32 = "C" 0x005F_F510; }
game_fn! { #[allow(dead_code)] fn free_vector(memory: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_data! { static info_pool: u8 = 0x00B0_D484; }
game_data! { static info_pool_block: *mut u8 = 0x00B0_D488; }
game_data! { static travel_network: [u32; 9] = 0x00AB_31A8; }
game_data! { static network_masks: [u32; 13] = 0x00AB_322C; }
game_data! { static network_manager: *mut u8 = 0x00B4_3D8C; }

unsafe fn row(sim: NetworkSim, x: i32) -> *mut u8 {
    unsafe { word(sim.rows(), x.wrapping_mul(12) as usize) as *mut u8 }
}
unsafe fn cell(sim: NetworkSim, x: i32, z: i32) -> *mut u8 {
    unsafe { row(sim, x).wrapping_add(z.wrapping_mul(4) as usize) }
}
unsafe fn wanted_mask(kind: u32) -> u32 {
    unsafe {
        let network = read_unaligned(travel_network().cast::<u32>().wrapping_add(kind as usize));
        read_unaligned(network_masks().cast::<u32>().wrapping_add(network as usize))
    }
}
unsafe fn boundary(this: *mut u8, x: i32, z: i32) -> *mut u8 {
    unsafe {
        let sim = TrafficSim::from_ptr(this);
        let (base, index) = if x < 0 { (sim.west_connections(), z) }
            else if x > sim.max_cell_x() { (sim.east_connections(), z) }
            else if z < 0 { (sim.north_connections(), x) }
            else { (sim.south_connections(), x) };
        base.wrapping_add(index.wrapping_mul(8) as usize)
    }
}

port! {
    /// Tests all nine travel modes and four rotations before accepting an edge's ferry bit.
    #[port(address = 0x0070_FA40, owner = "traffic", name = "cSC4TrafficSimulator::HasMapExit")]
    pub unsafe extern "thiscall" fn fc4p_0070fa40(this: *mut u8, x: i32, z: i32) -> u32 {
        unsafe {
            let (mut ix, mut iz) = (0, 0);
            let direction = inner_cell(this, x, z, &mut ix, &mut iz);
            if direction == -1 { return 0xFFFF_FF00; }
            let base = NetworkSim::from_ptr(this).connections().wrapping_add(
                ((ix.wrapping_shl(8) | iz).wrapping_mul(0x2E)) as usize);
            for mode in 0..9 {
                let connections = u32::from(read_unaligned(base.add(mode * 2).cast::<u16>()));
                for rotation in 0..4 {
                    if connections & 1u32.wrapping_shl((direction as u32).wrapping_add(rotation * 4)) != 0 {
                        return (direction as u32 & !255) | 1;
                    }
                }
            }
            let edge = boundary(this, x, z);
            (edge as u32 & !255) | u32::from(!edge.is_null() && edge.add(1).read() & 2 != 0)
        }
    }
}

register_port!(0x0070_FB30, fc4p_0070fb30, "cSC4TrafficSimulator::GetNetworkInfo", get_info, 12, [16, 12, 8]);
unsafe extern "fastcall" fn get_info(this: *mut u8, out: *mut Registers, kind: u32, x: i32, z: i32) {
    unsafe {
        let sim = NetworkSim::from_ptr(this);
        let row = row(sim, x);
        let mut node = word(row, z.wrapping_mul(4) as usize) as *mut u8;
        let mut result = Registers { eax: 0, ecx: sim.rows() as u32, edx: row as u32 };
        if !node.is_null() {
            result.ecx = kind;
            loop {
                result.edx = u32::from(NetworkInfo::from_ptr(node).kind());
                if result.edx == kind { result.eax = node as u32; break; }
                node = NetworkInfo::from_ptr(node).next();
                if node.is_null() { break; }
            }
        }
        out.write(result);
    }
}

unsafe fn new_info(kind: u32) -> *mut u8 {
    unsafe {
        let mut memory = word(read_unaligned(info_pool_block()), 8) as *mut u8;
        if memory.is_null() { memory = allocate_pool(info_pool()); }
        store(read_unaligned(info_pool_block()), 8, word(memory, 0));
        let block = read_unaligned(info_pool_block());
        store(block, 12, word(block, 12).wrapping_add(1));
        let info = NetworkInfo::from_ptr(memory);
        info.set_flags(0);
        info.set_connection_count(0);
        info.set_inverse_capacity(0);
        info.set_next(core::ptr::null_mut());
        info.set_kind(kind as u8);
        info.set_morning_subnetwork(0);
        info.set_evening_subnetwork(0);
        info.set_next(core::ptr::null_mut());
        memory
    }
}
port! {
    /// Finds a record by its full type argument, or appends a newly initialized pooled record.
    #[port(address = 0x0070_FB70, owner = "traffic", name = "EnsureNetworkInfo")]
    pub unsafe extern "thiscall" fn fc4p_0070fb70(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
        unsafe {
            let sim = NetworkSim::from_ptr(this);
            let mut node = word(cell(sim, x, z), 0) as *mut u8;
            if node.is_null() {
                let memory = new_info(kind);
                NetworkInfo::from_ptr(memory).set_connection_count(0);
                store(cell(sim, x, z), 0, memory as u32);
                return memory;
            }
            loop {
                let info = NetworkInfo::from_ptr(node);
                if u32::from(info.kind()) == kind { return node; }
                if info.next().is_null() { break; }
                node = info.next();
            }
            let memory = new_info(kind);
            let tail = NetworkInfo::from_ptr(node);
            tail.set_connection_count(0);
            tail.set_next(memory);
            memory
        }
    }
}
port! {
    /// Unlinks the first matching record before returning it to the game's pool.
    #[port(address = 0x0070_FC70, owner = "traffic", name = "cSC4TrafficSimulator::RemoveNetworkInfo")]
    pub unsafe extern "thiscall" fn fc4p_0070fc70(this: *mut u8, kind: u32, x: i32, z: i32) -> u32 {
        unsafe {
            let mut link = cell(NetworkSim::from_ptr(this), x, z);
            let mut node = word(link, 0) as *mut u8;
            let mut previous = node as u32;
            while !node.is_null() {
                let info = NetworkInfo::from_ptr(node);
                if u32::from(info.kind()) == kind {
                    store(link, 0, info.next() as u32);
                    return delete_info(node, 1);
                }
                previous = node as u32;
                link = node.add(0x14);
                node = info.next();
            }
            previous
        }
    }
}

register_port!(0x0070_FCE0, fc4p_0070fce0, "ClearNetworkSubnetworks", clear_subnetworks, 0, []);
unsafe extern "fastcall" fn clear_subnetworks(this: *mut u8, out: *mut Registers) {
    unsafe {
        let sim = NetworkSim::from_ptr(this);
        let bounds = TrafficSim::from_ptr(this);
        let mut eax = bounds.max_cell_x();
        sim.set_next_subnetwork(1);
        let mut x = 0i32;
        while x <= eax {
            let mut z = 0i32;
            while z <= bounds.max_cell_z() {
                let mut node = word(cell(sim, x, z), 0) as *mut u8;
                while !node.is_null() {
                    let info = NetworkInfo::from_ptr(node);
                    info.set_morning_subnetwork(0);
                    info.set_evening_subnetwork(0);
                    node = info.next();
                }
                z = z.wrapping_add(1);
            }
            eax = bounds.max_cell_x();
            x = x.wrapping_add(1);
        }
        out.write(Registers { eax: eax as u32, ecx: this as u32, edx: 0 });
    }
}

port! {
    /// Searches enabled network types at an outside cell and returns the first occupant found.
    #[port(address = 0x0071_04A0, owner = "traffic", name = "FindOutsideNetworkOccupant")]
    pub unsafe extern "thiscall" fn fc4p_007104a0(this: *mut u8, kind: u32, x: i32, z: i32) -> *mut u8 {
        unsafe {
            let sim = TrafficSim::from_ptr(this);
            if x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z() {
                return core::ptr::null_mut();
            }
            let mask = wanted_mask(kind);
            let manager = read_unaligned(network_manager());
            let edge = edge_record(this, x, z);
            if u32::from(read_unaligned(edge.cast::<u16>())) & mask != 0 {
                for network in 0..13 {
                    if u32::from(read_unaligned(edge.cast::<u16>())) & 1 << network & mask != 0 {
                        let occupant = vcall!(manager, 0x2C => fn(u32, i32, i32) -> *mut u8, network, x, z);
                        if !occupant.is_null() { return occupant; }
                    }
                }
            }
            core::ptr::null_mut()
        }
    }
}

port! {
    /// Returns the largest destination count among a lot's valid subnetwork IDs. Negative categories
    /// retain the original preceding-field indexing, and only categories above three are rejected.
    #[port(address = 0x0071_06C0, owner = "traffic", name = "cSC4TrafficSimulator::GetConnectedDestinationCount")]
    pub unsafe extern "fastcall" fn fc4p_007106c0(this: *mut u8, entry_edx: u32, lot: *mut u8, category: i32) -> u64 {
        unsafe {
            if category > 3 { return u64::from(entry_edx) << 32; }
            let mut vector = [0u32; 3];
            let holder = virtual_registers(lot, 0x0C, &[], vector.as_mut_ptr() as u32, word(lot, 0));
            let args = [holder.eax, 0x2A4F_9C74, vector.as_mut_ptr() as u32];
            let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x005F_F510), holder.ecx, &args);
            call.eax = holder.eax;
            call.edx = holder.edx;
            let mut result = registers(call);
            let mut maximum = 0;
            if result.eax as u8 != 0 {
                let count = (vector[1].wrapping_sub(vector[0]) as i32) >> 2;
                for index in 0..count {
                    let id = word(vector[0] as *mut u8, index.wrapping_mul(4) as usize);
                    let sim = NetworkSim::from_ptr(this);
                    let span = (sim.subnetworks_end() as u32).wrapping_sub(sim.subnetworks() as u32) as i32;
                    let high = ((i64::from(span) * -1_840_700_269) >> 32) as i32;
                    let quotient = high.wrapping_add(span) >> 4;
                    let count = quotient.wrapping_add((quotient as u32 >> 31) as i32);
                    result.edx = quotient as u32;
                    if id < count as u32 {
                        let offset = id.wrapping_mul(7).wrapping_add(category as u32).wrapping_mul(4).wrapping_add(12);
                        maximum = maximum.max(word(sim.subnetworks(), offset as usize));
                    }
                }
            }
            if vector[0] != 0 {
                let args = [vector[0]];
                let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), result.ecx, &args);
                call.eax = vector[0];
                call.edx = result.edx;
                result = registers(call);
            }
            (u64::from(result.edx) << 32) | u64::from(maximum)
        }
    }
}

port! {
    /// Looks up the byte-packed edge key and returns its 33-word density record or the game's zero record.
    #[port(address = 0x0071_1460, owner = "traffic", name = "cSC4TrafficSimulator::GetTrafficEdgeDensity")]
    pub unsafe extern "thiscall" fn fc4p_00711460(this: *mut u8, x: u32, z: u32, kind: u32, evening: u32) -> u64 {
        unsafe {
            let key = (u32::from(evening as u8 != 0) << 24) | ((kind & 255) << 16) | ((x & 255) << 8) | (z & 255);
            let sim = NetworkSim::from_ptr(this);
            let count = ((sim.buckets_end() as u32).wrapping_sub(sim.buckets() as u32) as i32 >> 2) as u32;
            let bucket: u32;
            // Keep the processor's unsigned division, including its zero-divisor exception.
            core::arch::asm!("div {count:e}", count = in(reg) count,
                inlateout("eax") key => _, inlateout("edx") 0u32 => bucket,
                options(nomem, nostack));
            let mut node = word(sim.buckets(), bucket.wrapping_mul(4) as usize) as *mut u8;
            while !node.is_null() && word(node, 4) != key { node = word(node, 0) as *mut u8; }
            let value = if node.is_null() { 0x00AB_31D0 } else { node.wrapping_add(8) as u32 };
            (u64::from(node as u32) << 32) | u64::from(value)
        }
    }
}

register_port!(0x0071_14E0, fc4p_007114e0, "cSC4TrafficSimulator::GetNeighborPoolPropertyHolder", neighbor_pool, 12, [16, 12, 8]);
unsafe extern "fastcall" fn neighbor_pool(this: *mut u8, out: *mut Registers, kind: u32, x: i32, z: i32) {
    unsafe {
        let mask = if kind == u32::MAX { u32::MAX } else { wanted_mask(kind) };
        let edge = boundary(this, x, z);
        let flags = u32::from(read_unaligned(edge.cast::<u16>()));
        let mut ecx = if x >= 0 && x <= TrafficSim::from_ptr(this).max_cell_x() && z < 0 {
            TrafficSim::from_ptr(this).north_connections() as u32
        } else if x < 0 { TrafficSim::from_ptr(this).west_connections() as u32 }
        else if x > TrafficSim::from_ptr(this).max_cell_x() { TrafficSim::from_ptr(this).east_connections() as u32 }
        else { TrafficSim::from_ptr(this).south_connections() as u32 };
        ecx = (ecx & 0xFFFF_0000) | flags;
        let mut result = Registers { eax: 0, ecx, edx: mask };
        if flags & mask != 0 || flags & 0x200 != 0 {
            let sim = NetworkSim::from_ptr(this);
            let id = edge.add(2).cast::<u16>();
            if read_unaligned(id) != 0 {
                let pool = this.add(0xB8);
                let args = [id as u32];
                let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x0070_EFC0), pool as u32, &args);
                call.eax = edge as u32;
                call.edx = mask;
                result = registers(call);
                if result.eax != word(pool, 0) {
                    result.eax = word(result.eax as *mut u8, 0x14);
                    out.write(result);
                    return;
                }
                write_unaligned(id, 0);
            }
            result.eax = sim.default_pool() as u32;
        }
        out.write(result);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0070fa40::PORT, fc4p_0070fb30::PORT, fc4p_0070fb70::PORT, fc4p_0070fc70::PORT,
    fc4p_0070fce0::PORT, fc4p_007104a0::PORT, fc4p_007106c0::PORT, fc4p_00711460::PORT, fc4p_007114e0::PORT,
];

#[cfg(test)]
mod tests;
