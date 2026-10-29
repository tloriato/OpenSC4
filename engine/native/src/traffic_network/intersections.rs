// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Intersection timers and the four incident edge records.

use super::{TileInfo, TrafficNetworkMap};
use crate::port::Port;
use crate::x87::F80;

layout! {
    /// An intersection edge occupies 20 bytes. The final two state bytes have separate meanings.
    pub struct IntersectionEdge {
        0x00 => kind / set_kind: u32,
        0x04 => green / set_green: u8,
        0x05 => warning / set_warning: u8,
        0x08 => green_duration / set_green_duration: u32,
        0x0C => red_duration / set_red_duration: u32,
        0x10 => remaining / set_remaining: f32,
    }
}

layout! {
    /// Four edge records followed by intersection metadata.
    pub struct Intersection {
        0x50 => active / set_active: u8,
        0x54 => props_begin / set_props_begin: *mut *mut u8,
        0x58 => props_end / set_props_end: *mut *mut u8,
        0x64 => dirty / set_dirty: u8,
    }
}

game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static warning_time: f32 = 0x00A8_FDD4; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_fn! {
    fn update_props(this: *mut u8, network: *mut u8, intersection: *mut u8, initial: u32)
        = "thiscall" 0x004A_E850;
}
game_fn! {
    fn configure_edge(this: *mut u8, network: *mut u8, neighbor: *mut u8,
        intersection: *mut u8, edge: *mut u8, direction: u32) = "thiscall" 0x004A_E9E0;
}

port! {
    /// Updates stoplight props from their directed edge and crossing props from the train crossing state.
    /// The city's slot 0x88 is called even for an empty prop array. The dirty byte is cleared after the pass.
    /// The intersection argument word holds the prop cursor and finishes at the current end pointer.
    #[port(address = 0x004A_E850, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::UpdateIntersectionProps", naked)]
    pub unsafe extern "C" fn fc4p_004ae850() {
        core::arch::naked_asm!("mov ecx, esp", "call {}", "ret 12", sym props_body)
    }
}

unsafe extern "fastcall" fn props_body(frame: *mut u32, _edx: u32) {
    unsafe {
        let intersection = frame.add(2).read() as *mut u8;
        let state = Intersection::from_ptr(intersection);
        let mut train_crossing = false;
        for direction in 0..4 {
            let edge = IntersectionEdge::from_ptr(intersection.add(direction * 20));
            if edge.kind() == 3 && edge.green() == 0 {
                train_crossing = true;
                break;
            }
        }
        vcall!(city().read(), 0x88 => fn());
        let mut cursor = state.props_begin();
        frame.add(2).write(cursor as u32);
        while cursor != state.props_end() {
            let prop = cursor.read_unaligned();
            let occupant = vcall!(prop, 0x0C => fn() -> *mut u8);
            let descriptor = vcall!(occupant, 0x18 => fn() -> *mut u8);
            let flag = vcall!(occupant, 0x58 => fn(u32) -> u32, 0x5002);
            let stoplight = flag as u8 != 0 && vcall!(descriptor, 0x0C => fn(u32) -> u32, 0x4A70_D491) as u8 != 0;
            if stoplight {
                let occupant = vcall!(prop, 0x0C => fn() -> *mut u8);
                let direct = vcall!(occupant, 0x80 => fn(u32) -> u32, 0x80);
                let orientation = vcall!(prop, 0x24 => fn() -> u32);
                let direction = if direct as u8 != 0 {
                    orientation
                } else {
                    ((orientation.wrapping_add(2) as i32) % 4) as u32
                };
                let edge = IntersectionEdge::from_ptr(intersection.wrapping_add(direction as u8 as usize * 20));
                if edge.kind() == 2 {
                    let value = if edge.green() == 0 {
                        2
                    } else if F80::from_f32(edge.remaining())
                        .compare(F80::from_f32(warning_time().read()))
                        .less()
                    {
                        1
                    } else {
                        0
                    };
                    vcall!(prop, 0x2C => fn(u32), value);
                }
            } else {
                let occupant = vcall!(prop, 0x0C => fn() -> *mut u8);
                let flag = vcall!(occupant, 0x58 => fn(u32) -> u32, 0x5003);
                if flag as u8 != 0 {
                    let old = vcall!(prop, 0x30 => fn() -> u32);
                    let initial = frame.add(3).read() as u8 != 0;
                    if train_crossing {
                        if old != 0 && old != 3 {
                            vcall!(prop, 0x2C => fn(u32), if initial { 0 } else { 3 });
                        }
                    } else if old != 2 && old != 1 {
                        vcall!(prop, 0x2C => fn(u32), u32::from(initial) + 1);
                    }
                }
            }
            cursor = (frame.add(2).read() as *mut *mut u8).wrapping_add(1);
            frame.add(2).write(cursor as u32);
        }
        state.set_dirty(0);
    }
}

port! {
    /// Subtracts elapsed time from stoplight edges. Negative timers toggle green and clear warning;
    /// timers below the warning threshold set warning once. Dirty intersections update their props once.
    /// Each timer subtraction also replaces the caller's intersection argument word with the stored timer.
    #[port(address = 0x004A_FB80, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::TickSingleIntersection", naked)]
    pub unsafe extern "C" fn fc4p_004afb80() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 12", sym tick_body)
    }
}

unsafe extern "fastcall" fn tick_body(this: *mut u8, frame: *mut u32) {
    unsafe {
        let network = frame.add(1).read() as *mut u8;
        let intersection = frame.add(2).read() as *mut u8;
        let elapsed = f32::from_bits(frame.add(3).read());
        let state = Intersection::from_ptr(intersection);
        for direction in 0..4 {
            let edge = IntersectionEdge::from_ptr(intersection.add(direction * 20));
            if edge.kind() == 2 {
                // The comparison reloads the stored single-precision timer.
                let remaining = (F80::from_f32(edge.remaining()) - F80::from_f32(elapsed)).to_f32();
                frame.add(2).write(remaining.to_bits());
                edge.set_remaining(remaining);
                if F80::from_f32(remaining).compare(F80::from_f32(zero().read())).less() {
                    edge.set_green(u8::from(edge.green() == 0));
                    edge.set_warning(0);
                    state.set_dirty(1);
                    let duration = if edge.green() != 0 {
                        edge.green_duration()
                    } else {
                        edge.red_duration()
                    };
                    edge.set_remaining(f32::from_bits(duration));
                } else if F80::from_f32(remaining)
                    .compare(F80::from_f32(warning_time().read()))
                    .less()
                    && edge.warning() == 0
                {
                    edge.set_warning(1);
                    state.set_dirty(1);
                }
            }
        }
        if state.dirty() != 0 {
            update_props(this, network, intersection, 0);
        }
    }
}

port! {
    /// Initializes four incident edges when the network has an intersection interface, updates props,
    /// and marks the map changed. The third argument is unused.
    /// The caller's first two words hold the direction byte and moving edge pointer. Direction words sent
    /// to the callees retain the upper bytes of the network argument.
    #[port(address = 0x004A_FC20, owner = "trafficnetwork", name = "initialize traffic intersection", naked)]
    pub unsafe extern "C" fn fc4p_004afc20() {
        core::arch::naked_asm!("mov edx, esp", "call {}", "ret 12", sym initialize_body)
    }
}

unsafe extern "fastcall" fn initialize_body(this: *mut u8, frame: *mut u32) {
    unsafe {
        let network = frame.add(1).read() as *mut u8;
        let node = frame.add(2).read() as *mut u8;
        let interface = vcall!(network, 0xC4 => fn() -> *mut u8);
        if interface.is_null() {
            return;
        }
        let intersection = node.add(8);
        let state = Intersection::from_ptr(intersection);
        state.set_active(0);
        state.set_dirty(1);
        frame.add(1).cast::<u8>().write(0);
        frame.add(2).write(intersection as u32);
        loop {
            let direction = frame.add(1).read();
            let tile = vcall!(this, 0x3C => fn(*mut u8, u32, u32, u32) -> *mut u8,
                    network, direction, 0, 0);
            let neighbor = if tile.is_null() {
                core::ptr::null_mut()
            } else {
                TileInfo::from_ptr(tile).network()
            };
            let direction = frame.add(1).read();
            let edge = frame.add(2).read() as *mut u8;
            configure_edge(this, network, neighbor, intersection, edge, direction);
            let direction = frame.add(1).cast::<u8>().read().wrapping_add(1);
            let edge = frame.add(2).read().wrapping_add(20);
            frame.add(1).cast::<u8>().write(direction);
            frame.add(2).write(edge);
            if direction >= 4 {
                break;
            }
        }
        update_props(this, network, intersection, 1);
        TrafficNetworkMap::from_ptr(this).set_intersections_changed(1);
    }
}

pub const PORTS: &[Port] = &[fc4p_004ae850::PORT, fc4p_004afb80::PORT, fc4p_004afc20::PORT];

#[cfg(test)]
mod tests;
