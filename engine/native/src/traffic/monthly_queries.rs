// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Monthly maintenance, strategy percentages, ferry routes and the route hash.

use super::late_containers::{call, register_entry};
use super::*;
use crate::abi::Conv;

layout! {
    /// Fields used by the monthly and public query entries.
    pub struct MonthlySim {
        0x104 => ferries / set_ferries: *mut u8,
        0x1E4 => congestion / set_congestion: f32,
        0x40C => road_funding / set_road_funding: u32,
        0x410 => transit_funding / set_transit_funding: u32,
        0x424 => type_counts / set_type_counts: *mut u32,
        0x43C => ferry_count / set_ferry_count: u32,
    }
}
layout! {
    /// Bucket-vector bounds and element count of the route hash.
    pub struct RouteHash {
        0x04 => buckets / set_buckets: *mut u32,
        0x08 => end / set_end: *mut u32,
        0x10 => count / set_count: u32,
    }
}

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(memory: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn clone_routes(this: *mut u8, source: *mut u8) = "thiscall" 0x006D_6C30; }
game_fn! { #[allow(dead_code)] fn clone_bytes(this: *mut u8, source: *mut u8) = "thiscall" 0x0045_9E80; }
game_fn! { #[allow(dead_code)] fn clear_routes(this: *mut u8) = "thiscall" 0x006F_CBD0; }
game_fn! { #[allow(dead_code)] fn resize_percentages(this: *mut u8, size: u32, value: *const u32) = "thiscall" 0x0071_69C0; }
game_fn! { fn road_funding(this: *mut u8, funding: *mut u8) = "thiscall" 0x0071_7D30; }
game_fn! { fn transit_funding(this: *mut u8, funding: *mut u8) = "thiscall" 0x0071_7E00; }
game_fn! { fn strikes(this: *mut u8) = "thiscall" 0x0070_DBA0; }
game_fn! { fn accidents(this: *mut u8) = "thiscall" 0x0071_84E0; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static hundred: f32 = 0x00A8_0AA0; }
game_data! { static commute_scale: f32 = 0x00AB_32C0; }
game_data! { static percent_scale: f32 = 0x00A8_23A4; }

/// Unsigned DIV, including the processor exception when the bucket count is zero.
pub(super) fn unsigned_division(value: u32, divisor: u32) -> (u32, u32) {
    let mut quotient = value;
    let remainder: u32;
    unsafe {
        core::arch::asm!("xor edx, edx", "div {divisor:e}", divisor = in(reg) divisor,
            inout("eax") quotient, out("edx") remainder, options(nomem, nostack));
    }
    (quotient, remainder)
}

register_entry!(
    0x0071_8800,
    fc4p_00718800,
    "InsertTrafficRouteHash",
    insert_route,
    8,
    [12, 8]
);
unsafe extern "fastcall" fn insert_route(this: *mut u8, out: *mut Registers, result: *mut u8, source: *mut u8) {
    unsafe {
        let hash = RouteHash::from_ptr(this);
        let buckets = hash.buckets();
        let size = (hash.end() as u32).wrapping_sub(buckets as u32) as i32 >> 2;
        let key = word(source, 0);
        let (quotient, bucket) = unsigned_division(key, size as u32);
        let head = buckets.wrapping_add(bucket as usize).read_unaligned() as *mut u8;
        let mut node = head;
        let mut r = out.read();
        r.edx = buckets as u32;
        while !node.is_null() && word(node, 4) != key {
            node = word(node, 0) as *mut u8;
        }
        let inserted = node.is_null();
        if inserted {
            r.eax = quotient;
            r.ecx = 0;
            call(0x0090_CF54, Conv::Cdecl, &[0x3C], &mut r);
            node = r.eax as *mut u8;
            r.eax = (node as u32).wrapping_add(4);
            store(node, 0, 0);
            if r.eax != 0 {
                r.edx = word(source, 0);
                r.ecx = (source as u32).wrapping_add(4);
                store(node, 4, r.edx);
                r.ecx = (node as u32).wrapping_add(8);
                call(0x006D_6C30, Conv::Thiscall, &[source.wrapping_add(4) as u32], &mut r);
            }
            store(node, 0, head as u32);
            hash.buckets()
                .wrapping_add(bucket as usize)
                .write_unaligned(node as u32);
            hash.set_count(hash.count().wrapping_add(1));
        } else {
            r.ecx = node as u32;
        }
        store(result, 0, node as u32);
        store(result, 4, this as u32);
        result.add(8).write(u8::from(inserted));
        r.eax = result as u32;
        out.write(r);
    }
}

register_entry!(0x0071_88A0, fc4p_007188a0, "EraseTrafficRouteHash", erase_route, 4, [8]);
unsafe extern "fastcall" fn erase_route(this: *mut u8, out: *mut Registers, iterator: *mut u8) {
    unsafe {
        let mut r = out.read();
        r.eax = iterator as u32;
        r.ecx = word(iterator, 0);
        if r.ecx != 0 {
            let hash = RouteHash::from_ptr(this);
            let buckets = hash.buckets();
            let size = (hash.end() as u32).wrapping_sub(buckets as u32) as i32 >> 2;
            let key = word(r.ecx as *mut u8, 4);
            let (_, bucket) = unsigned_division(key, size as u32);
            r.edx = bucket;
            let slot = buckets.wrapping_add(bucket as usize);
            let head = slot.read_unaligned() as *mut u8;
            r.eax = slot as u32;
            let mut victim = head;
            if head as u32 == r.ecx {
                r.ecx = word(head, 0);
                slot.write_unaligned(r.ecx);
            } else {
                let mut previous = head;
                victim = word(previous, 0) as *mut u8;
                while !victim.is_null() && victim as u32 != r.ecx {
                    previous = victim;
                    victim = word(victim, 0) as *mut u8;
                }
                if victim.is_null() {
                    out.write(r);
                    return;
                }
                r.edx = word(victim, 0);
                store(previous, 0, r.edx);
            }
            r.eax = word(victim, 0x24);
            if r.eax != 0 {
                call(0x0090_CF63, Conv::Cdecl, &[r.eax], &mut r);
            }
            call(0x0090_CF63, Conv::Cdecl, &[victim as u32], &mut r);
            r.eax = hash.count().wrapping_sub(1);
            hash.set_count(r.eax);
        }
        out.write(r);
    }
}

register_entry!(
    0x0071_8AC0,
    fc4p_00718ac0,
    "cSC4TrafficSimulator::DoMonthlySimulation",
    monthly,
    0,
    []
);
unsafe extern "fastcall" fn monthly(this: *mut u8, out: *mut Registers) {
    unsafe {
        let sim = TrafficSim::from_ptr(this);
        let fields = MonthlySim::from_ptr(this);
        let funding = vcall!(sim.roads(), 0x28 => fn() -> *mut u8);
        road_funding(this, funding);
        let funding = vcall!(sim.transit(), 0x28 => fn() -> *mut u8);
        transit_funding(this, funding);
        strikes(this);
        accidents(this);
        let funding = vcall!(sim.roads(), 0x28 => fn() -> *mut u8);
        fields.set_road_funding(word(funding, 0));
        let funding = vcall!(sim.transit(), 0x28 => fn() -> *mut u8);
        let congestion = F80::from_f32(fields.congestion());
        let transit = word(funding, 0);
        let congestion = multiply_float(congestion, *hundred());
        fields.set_transit_funding(transit);
        let recorder = *history();
        let table = word(recorder, 0);
        let point = congestion.ftol() as u32;
        let f = core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32)>(
            (table as *const usize).add(0x30 / 4).read(),
        );
        f(recorder, 0xEA5B_A01D, point);
        let commute = multiply_float(F80::from_f32(sim.average_commute()), *commute_scale());
        let recorder = *history();
        let target = crate::abi::vslot(recorder, 0x30);
        let point = commute.ftol() as u32;
        core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, u32, u32)>(target)(
            recorder,
            0x8A5D_4116,
            point,
        );
        for i in 0..9 {
            let value = fields.type_counts().add(i).read_unaligned();
            vcall!(*history(), 0x30 => fn(u32, u32), 0xABB8_FAACu32.wrapping_add(i as u32), value);
        }
        let value = fields.ferry_count();
        let recorder = *history();
        out.write(virtual_registers(
            recorder,
            0x30,
            &[0xEC0B_DBF8, value],
            value,
            word(recorder, 0),
        ));
    }
}

// Supply the original argument words to entries that reuse their caller-owned stack slots.
macro_rules! frame_entry {
    ($address:literal, $export:ident, $name:literal, $body:ident, $pop:literal) => {
        port! {
            #[port(address = $address, owner = "traffic", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push ebp", "mov ebp, esp", "sub esp, 12",
                    "mov [esp], eax", "mov [esp + 4], ecx", "mov [esp + 8], edx",
                    "lea edx, [esp]", "lea eax, [ebp + 8]", "push eax", "call {body}",
                    "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
                    "mov esp, ebp", "pop ebp", concat!("ret ", stringify!($pop)), body = sym $body,
                );
            }
        }
    };
}
pub(super) use frame_entry;

frame_entry!(
    0x0071_8BB0,
    fc4p_00718bb0,
    "cSC4TrafficSimulator::GetTravelStrategyPercentages",
    percentages,
    8
);
unsafe extern "fastcall" fn percentages(this: *mut u8, out: *mut Registers, args: *mut u32) {
    unsafe {
        let vector = args.add(1).read() as *mut u8;
        let zero = 0u32;
        let mut r = out.read();
        r.ecx = vector as u32;
        call(0x0071_69C0, Conv::Thiscall, &[3, &zero as *const u32 as u32], &mut r);
        let kind = args.read();
        let offset = kind.wrapping_mul(3).wrapping_add(0xC5C) as usize;
        let first = u32::from(this.wrapping_add(offset).read());
        args.add(1).write(first);
        let destination = word(vector, 0) as *mut u8;
        let value = multiply_float(F80::from_i32(first as i32), *percent_scale()).to_f32();
        store(destination, 0, value.to_bits());
        let second = u32::from(this.wrapping_add(offset.wrapping_add(1)).read());
        let destination = word(vector, 0) as *mut u8;
        args.add(1).write(second);
        let value = multiply_float(F80::from_i32(second as i32), *percent_scale()).to_f32();
        store(destination, 4, value.to_bits());
        let third = u32::from(this.wrapping_add(offset.wrapping_add(2)).read());
        let last = word(vector, 0) as *mut u8;
        args.add(1).write(third);
        let value = multiply_float(F80::from_i32(third as i32), *percent_scale()).to_f32();
        store(last, 8, value.to_bits());
        out.write(Registers {
            eax: third,
            ecx: 0,
            edx: destination as u32,
        });
    }
}

register_entry!(
    0x0071_8C40,
    fc4p_00718c40,
    "cSC4TrafficSimulator::GetAllFerryRoutes",
    ferry_routes,
    4,
    [8]
);
unsafe extern "fastcall" fn ferry_routes(this: *mut u8, out: *mut Registers, destination: *mut u8) {
    unsafe {
        let mut r = out.read();
        r.ecx = destination as u32;
        call(0x006F_CBD0, Conv::Thiscall, &[], &mut r);
        let sentinel = MonthlySim::from_ptr(this).ferries();
        let mut terminal = word(sentinel, 0) as *mut u8;
        while terminal != sentinel {
            for offset in [0x18, 0x1C] {
                let header = word(terminal, offset) as *mut u8;
                let mut route = word(header, 0) as *mut u8;
                while route != header {
                    r.eax = destination as u32;
                    let head = word(destination, 0) as *mut u8;
                    call(0x0090_CF54, Conv::Cdecl, &[0x14], &mut r);
                    let node = r.eax as *mut u8;
                    r.ecx = (node as u32).wrapping_add(8);
                    if r.ecx != 0 {
                        r.edx = route.wrapping_add(0x14) as u32;
                        call(0x0045_9E80, Conv::Thiscall, &[r.edx], &mut r);
                    }
                    r.eax = word(head, 4);
                    store(node, 0, head as u32);
                    store(node, 4, r.eax);
                    store(r.eax as *mut u8, 0, node as u32);
                    store(head, 4, node as u32);
                    route = word(route, 0) as *mut u8;
                }
            }
            terminal = word(terminal, 0) as *mut u8;
        }
        r.eax = terminal as u32;
        out.write(r);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00718800::PORT,
    fc4p_007188a0::PORT,
    fc4p_00718ac0::PORT,
    fc4p_00718bb0::PORT,
    fc4p_00718c40::PORT,
];

#[cfg(test)]
mod tests;
