// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Trip search configuration.

use super::*;
use super::network::registers;
use crate::abi::{Conv, RawCall};

game_fn! { #[allow(dead_code)] fn route_iterator(this: *mut u8, route: *mut u8) -> u64 = "thiscall" 0x006D_64B0; }
game_fn! { #[allow(dead_code)] fn route_next(this: *mut u8, step: *mut u8) -> u64 = "thiscall" 0x006D_6510; }

unsafe fn next(iterator: *mut u8, step: *mut u8, incoming: Registers) -> Registers {
    let args = [step as u32];
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006D_6510), iterator as u32, &args);
    call.eax = incoming.eax;
    call.edx = incoming.edx;
    unsafe { registers(call) }
}
unsafe fn inside(sim: TrafficSim, x: i32, z: i32) -> bool {
    x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z()
}
unsafe fn route_connections(sim: TrafficSim, x: i32, z: i32) -> *mut u8 {
    sim.intersections().wrapping_add((x.wrapping_shl(8) | z).wrapping_mul(46) as usize)
}
fn low_byte(value: u32, byte: u8) -> u32 { (value & !255) | u32::from(byte) }

port! {
    /// Keeps the iterator and its step at their original stack offsets, including the iterator pointer
    /// a completed route can leave in ECX.
    #[port(address = 0x0070_E320, owner = "traffic", name = "cSC4TrafficSimulator::ConfirmRoute", naked)]
    pub unsafe extern "C" fn fc4p_0070e320() {
        core::arch::naked_asm!(
            "sub esp, 0x28", "push ebp", "mov ebp, esp", "sub esp, 12", "lea edx, [esp]",
            "lea eax, [ebp + 8]", "push eax", "lea eax, [ebp + 20]", "push eax",
            "push dword ptr [ebp + 48]", "call {body}",
            "mov eax, [ebp - 12]", "mov ecx, [ebp - 8]", "mov edx, [ebp - 4]",
            "mov esp, ebp", "pop ebp", "add esp, 0x28", "ret 4", body = sym confirm_route,
        );
    }
}
unsafe extern "fastcall" fn confirm_route(this: *mut u8, out: *mut Registers, route: *mut u8, iterator: *mut u8, step_buffer: *mut u8) {
    unsafe {
        let sim = TrafficSim::from_ptr(this);
        let strike = u8::from(sim.strike_state() == 1);
        let args = [route as u32];
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x006D_64B0), iterator as u32, &args);
        call.eax = route as u32;
        let mut result = registers(call);
        result = next(iterator, step_buffer, result);
        if result.eax as u8 == 0 { out.write(result); return; }
        let step: [u8; 10] = read_unaligned(step_buffer.cast());
        let x = i32::from(read_unaligned(step.as_ptr().cast::<i16>()));
        let z = i32::from(read_unaligned(step.as_ptr().add(2).cast::<i16>()));
        result.eax = x as u32;
        result.ecx = z as u32;
        if inside(sim, x, z) && step[8] != 255 {
            let direction = u32::from(read_unaligned(step.as_ptr().add(8).cast::<u16>()));
            let connections = route_connections(sim, x, z);
            result.edx = low_byte(result.edx, step[4]);
            result.eax = connections as u32;
            result.ecx = sim.intersections() as u32;
            let mut accepted = false;
            for rotation in 0..4 {
                if step[4] == step[5] {
                    result.eax = 1u32.wrapping_shl(direction.wrapping_add(rotation * 4));
                    result.ecx = u32::from(read_unaligned(connections.add(usize::from(step[4]) * 2).cast::<u16>()));
                    accepted = result.eax & result.ecx != 0;
                } else {
                    result.eax = u32::from(read_unaligned(connections.add(18 + usize::from(step[4]) * 2).cast::<u16>()));
                    let shift = step[8].wrapping_shl(2);
                    result.ecx = low_byte(result.ecx, step[5]);
                    result.eax = low_byte(result.eax.wrapping_shr(u32::from(shift) & 31),
                        (result.eax.wrapping_shr(u32::from(shift) & 31) & 15) as u8);
                    accepted = result.eax as u8 == step[5];
                }
                if accepted { break; }
            }
            if !accepted { result.eax &= !255; out.write(result); return; }
        }
        loop {
            result = next(iterator, step_buffer, result);
            if result.eax as u8 == 0 {
                result.eax = low_byte(result.eax, 1);
                out.write(result);
                return;
            }
            let step: [u8; 10] = read_unaligned(step_buffer.cast());
            let x = i32::from(read_unaligned(step.as_ptr().cast::<i16>()));
            let z = i32::from(read_unaligned(step.as_ptr().add(2).cast::<i16>()));
            result.eax = x as u32;
            result.edx = z as u32;
            if !inside(sim, x, z) || step[8] == 255 { continue; }
            let direction = u32::from(read_unaligned(step.as_ptr().add(8).cast::<u16>()));
            result.ecx = (result.ecx & 0xFFFF_0000) | direction;
            let connections = route_connections(sim, x, z);
            result.eax = connections as u32;
            result.edx = low_byte(result.edx, step[4]);
            let accepted = if step[4] == step[5] {
                let bit = 1u32.wrapping_shl(direction.wrapping_add(u32::from(step[7]) * 4));
                result.edx = u32::from(step[4]);
                result.ecx = u32::from(read_unaligned(connections.add(usize::from(step[4]) * 2).cast::<u16>()));
                if result.ecx & bit != 0 { true } else {
                    let flags = connections.add(44).read();
                    result.eax = low_byte(result.eax, flags);
                    if flags & 2 != 0 { true }
                    else if flags & 4 == 0 { false }
                    else { result.eax = low_byte(result.eax, strike); strike == 0 }
                }
            } else {
                result.ecx = low_byte(result.ecx, strike);
                if strike != 0 { false } else {
                    result.ecx = low_byte(result.ecx, step[8].wrapping_shl(2));
                    result.edx = u32::from(step[4]);
                    let connections = u32::from(read_unaligned(connections.add(18 + usize::from(step[4]) * 2).cast::<u16>()));
                    let shifted = connections.wrapping_shr(u32::from(step[8].wrapping_shl(2)) & 31);
                    result.eax = (result.eax & 0xFFFF_0000) | (shifted & !255) | (shifted & 15);
                    result.eax as u8 == step[5]
                }
            };
            if !accepted { result.eax &= !255; out.write(result); return; }
        }
    }
}

layout! {
    /// Current trip's two search engines and source rectangle provider.
    pub struct TripContext {
        0x08 => outbound / set_outbound: *mut u8,
        0x0C => inbound / set_inbound: *mut u8,
        0x10 => source / set_source: *mut u8,
        0x45 => continue_search / set_continue_search: u8,
        0x4D => reversible / set_reversible: u8,
        0x50 => work_time / set_work_time: u32,
        0x54 => freight / set_freight: u32,
    }
}
layout! {
    /// Fields of a trip used to configure a search.
    pub struct Trip {
        0x18 => traffic / set_traffic: u32,
        0x1C => kind / set_kind: u8,
        0x1D => strategy / set_strategy: u8,
        0x1E => flags / set_flags: u8,
        0x1F => status / set_status: u8,
        0x20 => destination_x / set_destination_x: i16,
        0x22 => destination_z / set_destination_z: i16,
    }
}
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }

port! {
    /// Configures the outbound search and both work limits, preserving the original call order.
    /// Re-routing targets a lot rectangle when one exists, otherwise the destination cell. Commuters
    /// select a strategy's cost array or forced start type; freight has its own mode and flags.
    #[port(address = 0x0070_EC40, owner = "traffic", name = "cSC4TrafficSimulator::SetupTrip")]
    pub unsafe extern "thiscall" fn fc4p_0070ec40(this: *mut u8, context: *mut u8, trip: *mut u8) {
        unsafe {
            let cx = TripContext::from_ptr(context);
            let trip = Trip::from_ptr(trip);
            let sim = TrafficSim::from_ptr(this);
            vcall!(cx.outbound(), 0x48 => fn(u32), trip.traffic());
            if trip.status() == 1 {
                vcall!(cx.outbound(), 0x24 => fn(u32), 3);
                let (x, z) = (i32::from(trip.destination_x()), i32::from(trip.destination_z()));
                let lot = if x >= sim.min_cell_x() && z >= sim.min_cell_z() && x <= sim.max_cell_x() && z <= sim.max_cell_z() {
                    vcall!(read_unaligned(lot_manager()), 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 0)
                } else { core::ptr::null_mut() };
                let mut rect = [0i32; 4];
                if lot.is_null() {
                    let (x, z) = (i32::from(trip.destination_x()), i32::from(trip.destination_z()));
                    rect = [x, z, x, z];
                } else {
                    vcall!(lot, 0x38 => fn(*mut i32), rect.as_mut_ptr());
                }
                vcall!(cx.outbound(), 0x50 => fn(i32, i32, i32, i32), rect[0], rect[1], rect[2], rect[3]);
            } else {
                vcall!(cx.outbound(), 0x24 => fn(u32), 0);
                vcall!(cx.outbound(), 0x30 => fn());
                let bits = if trip.flags() & 2 != 0 && trip.kind() == 0 { 7 }
                    else { 1u32.wrapping_shl(u32::from(trip.kind())) };
                vcall!(cx.outbound(), 0x28 => fn(u32), bits);
            }
            vcall!(cx.outbound(), 0x54 => fn(u32), cx.work_time());
            vcall!(cx.inbound(), 0x54 => fn(u32), cx.work_time());
            let mut rect = [0i32; 4];
            let source = vcall!(cx.source(), 0x14 => fn(*mut i32) -> *mut i32, rect.as_mut_ptr());
            let rect: [i32; 4] = read_unaligned(source.cast());
            vcall!(cx.outbound(), 0x4C => fn(i32, i32, i32, i32), rect[0], rect[1], rect[2], rect[3]);
            vcall!(cx.outbound(), 0x1C => fn());
            vcall!(cx.outbound(), 0x3C => fn());
            vcall!(cx.outbound(), 0x40 => fn(u32), 0);
            vcall!(cx.outbound(), 0x44 => fn(u32), sim.commute_maximum().to_bits());
            match trip.kind() {
                0..=2 => {
                    let continuing = cx.continue_search();
                    cx.set_freight(0);
                    cx.set_reversible(1);
                    if continuing != 0 { vcall!(cx.outbound(), 0x14 => fn(u32), 4); }
                    vcall!(cx.outbound(), 0x34 => fn(u32, u32), 0x1CF, 0);
                    if trip.flags() & 4 != 0 {
                        vcall!(cx.outbound(), 0x34 => fn(u32, u32), 1u32.wrapping_shl(u32::from(trip.strategy())), 1);
                    } else {
                        vcall!(cx.outbound(), 0x34 => fn(u32, u32), 3, 1);
                        let offset = match trip.strategy() { 0 => Some(0xC9C), 1 => Some(0xCC0), 2 => Some(0xC78), _ => None };
                        if let Some(offset) = offset { vcall!(cx.outbound(), 0x40 => fn(*mut u8), this.add(offset)); }
                    }
                }
                3 => {
                    cx.set_freight(1);
                    vcall!(cx.outbound(), 0x14 => fn(u32), 1);
                    vcall!(cx.outbound(), 0x34 => fn(u32, u32), 0x30, 1);
                    cx.set_reversible(0);
                }
                _ => {}
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070e320::PORT, fc4p_0070ec40::PORT];

#[cfg(test)]
mod tests;
