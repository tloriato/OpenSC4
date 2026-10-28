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

//! Coordinate conversion, neighborhoods, wire edits, and powered queries.

use super::*;
use core::ptr::{read_unaligned, write_unaligned};

game_fn! { fn rect_set(bits: *mut u32, width: u32, rect: *mut Rect) = "C" 0x0060_4180; }
game_fn! { fn rect_clear(bits: *mut u32, width: u32, rect: *mut Rect) = "C" 0x0060_4260; }
game_fn! { fn pixel_test(bits: *mut u32, width: u32, x: i32, z: i32) -> u64 = "C" 0x0060_3DD0; }
game_fn! { fn expand(this: *mut u8, rect: *mut Rect) = "thiscall" 0x006E_AE20; }
game_fn! { fn set_wire(this: *mut u8, rect: *mut Rect) = "thiscall" 0x006E_AFD0; }
game_fn! { fn dirty_rect(this: *mut u8, rect: *mut Rect) -> u32 = "thiscall" 0x006E_B230; }
game_data! { static global_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }

port! {
    /// Convert each coordinate using the reciprocal cell width and the game's truncating helper.
    /// Return the second helper's full EDX:EAX result, and reload the city before the second call.
    #[port(address = 0x006E_AC40, owner = "power", name = "cSC4PowerSimulator::PositionToCell")]
    pub unsafe extern "thiscall" fn fc4p_006eac40(this: *mut u8, x: f32, z: f32, ox: *mut i32, oz: *mut i32) -> i64 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let width = vcall_st0!(sim.city(), 0x16C);
            let cx = multiply_float(divide_into_float(width, 1.0), x).ftol();
            ox.write_unaligned(cx as i32);
            let width = vcall_st0!(sim.city(), 0x170);
            let cz = multiply_float(divide_into_float(width, 1.0), z).ftol();
            oz.write_unaligned(cz as i32);
            cz
        }
    }
}
port! {
    /// Expand a point by the tuned radius. Upper bounds clamp through a second size query.
    #[port(address = 0x006E_AD70, owner = "power", name = "cSC4PowerSimulator::GetNeighborhoodOfPoint")]
    pub unsafe extern "thiscall" fn fc4p_006ead70(this: *mut u8, x: u32, z: u32, rect: *mut Rect) {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let r = sim.radius();
            write_unaligned(core::ptr::addr_of_mut!((*rect).x0), if x > r { x.wrapping_sub(r) } else { 0 });
            let r = sim.radius();
            write_unaligned(core::ptr::addr_of_mut!((*rect).z0), if z > r { z.wrapping_sub(r) } else { 0 });
            let hi = x.wrapping_add(sim.radius());
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            let hi = if hi < width { hi } else { vcall!(sim.city(), 0x174 => fn() -> u32).wrapping_sub(1) };
            write_unaligned(core::ptr::addr_of_mut!((*rect).x1), hi);
            let hi = z.wrapping_add(sim.radius());
            let height = vcall!(sim.city(), 0x178 => fn() -> u32);
            let hi = if hi < height { hi } else { vcall!(sim.city(), 0x178 => fn() -> u32).wrapping_sub(1) };
            write_unaligned(core::ptr::addr_of_mut!((*rect).z1), hi);
        }
    }
}
port! {
    /// Widen inclusive bounds in place, reading each field when it is reached. Arithmetic wraps.
    #[port(address = 0x006E_AE20, owner = "power", name = "cSC4PowerSimulator::ExpandRectToIncludeNeighborhood")]
    pub unsafe extern "thiscall" fn fc4p_006eae20(this: *mut u8, rect: *mut Rect) {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let lo = read_unaligned(core::ptr::addr_of!((*rect).x0));
            let r = sim.radius();
            write_unaligned(core::ptr::addr_of_mut!((*rect).x0), if lo > r { lo.wrapping_sub(r) } else { 0 });
            let lo = read_unaligned(core::ptr::addr_of!((*rect).z0));
            let r = sim.radius();
            write_unaligned(core::ptr::addr_of_mut!((*rect).z0), if lo > r { lo.wrapping_sub(r) } else { 0 });
            let hi = read_unaligned(core::ptr::addr_of!((*rect).x1)).wrapping_add(sim.radius());
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            let hi = if hi < width { hi } else { vcall!(sim.city(), 0x174 => fn() -> u32).wrapping_sub(1) };
            write_unaligned(core::ptr::addr_of_mut!((*rect).x1), hi);
            let hi = read_unaligned(core::ptr::addr_of!((*rect).z1)).wrapping_add(sim.radius());
            let height = vcall!(sim.city(), 0x178 => fn() -> u32);
            let hi = if hi < height { hi } else { vcall!(sim.city(), 0x178 => fn() -> u32).wrapping_sub(1) };
            write_unaligned(core::ptr::addr_of_mut!((*rect).z1), hi);
        }
    }
}
port! {
    /// Pass a local copy of the inclusive rectangle to the bitmap setter after querying the city width.
    #[port(address = 0x006E_AFD0, owner = "power", name = "cSC4PowerSimulator::SetWireBits")]
    pub unsafe extern "thiscall" fn fc4p_006eafd0(this: *mut u8, rect: *const Rect) {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let mut local = rect.read_unaligned();
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            rect_set(sim.wire(), width, &mut local);
        }
    }
}
port! {
    /// Pass a local copy to the bitmap clearer, reloading the wire pointer after the city call.
    #[port(address = 0x006E_B020, owner = "power", name = "cSC4PowerSimulator::ResetWireBits")]
    pub unsafe extern "thiscall" fn fc4p_006eb020(this: *mut u8, rect: *const Rect) {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let mut local = rect.read_unaligned();
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            rect_clear(sim.wire(), width, &mut local);
        }
    }
}
port! {
    /// Query the current powered bitmap through the shared pixel helper. EDX retains that helper's result.
    #[port(address = 0x006E_B070, owner = "power", name = "cSC4PowerSimulator::IsPowered")]
    pub unsafe extern "thiscall" fn fc4p_006eb070(this: *mut u8, x: i32, z: i32) -> u64 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let width = vcall!(sim.city(), 0x174 => fn() -> u32);
            pixel_test(sim.powered(), width, x, z)
        }
    }
}
port! {
    /// Read an object's cell position, then clamp both signed coordinates to the global city's bounds.
    /// A null object leaves both destinations alone.
    #[port(address = 0x006E_B280, owner = "power", name = "ClampObjectCell")]
    pub unsafe extern "stdcall" fn fc4p_006eb280(object: *mut u8, x: *mut i32, z: *mut i32) {
        unsafe {
            if object.is_null() { return; }
            let xmax = vcall!(*global_city(), 0x174 => fn() -> i32).wrapping_sub(1);
            let zmax = vcall!(*global_city(), 0x178 => fn() -> i32).wrapping_sub(1);
            vcall!(object, 0x18 => fn(*mut i32, *mut i32), x, z);
            if x.read_unaligned() < 0 { x.write_unaligned(0); }
            if z.read_unaligned() < 0 { z.write_unaligned(0); }
            if x.read_unaligned() > xmax { x.write_unaligned(xmax); }
            if z.read_unaligned() > zmax { z.write_unaligned(zmax); }
        }
    }
}
port! {
    /// Expand the changed area, add conducting cells for a nonzero low-byte zone, then queue the rectangle.
    #[port(address = 0x006E_B350, owner = "power", name = "cSC4PowerSimulator::CellChanged")]
    pub unsafe extern "thiscall" fn fc4p_006eb350(this: *mut u8, x0: u32, z0: u32, x1: u32, z1: u32) -> u32 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let mut rect = Rect { x0, z0, x1, z1 };
            expand(this, &mut rect);
            if vcall!(sim.zones(), 0x30 => fn(u32, u32) -> u32, x0, z0) as u8 != 0 {
                set_wire(this, &mut rect);
            }
            (dirty_rect(this, &mut rect) & !255) | 1
        }
    }
}
// The flag methods may leave EDX untouched, so pass the exact register values around the calls.
unsafe fn lot_call(object: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    use crate::abi::{call_raw, vslot, Conv, RawCall};
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        call.eax = eax;
        call.edx = edx;
        call_raw(&mut call);
        (u64::from(call.edx) << 32) | u64::from(call.eax)
    }
}
port! {
    /// A missing lot is unpowered. Otherwise test its powered flag first, then the inverse needs-power flag.
    #[port(address = 0x006E_B3D0, owner = "power", name = "cSC4PowerSimulator::IsOccupantPowered")]
    pub unsafe extern "thiscall" fn fc4p_006eb3d0(_this: *mut u8, occupant: *mut u8) -> u64 {
        unsafe {
            let manager = *lot_manager();
            let mut result = lot_call(manager, 0x38, manager.cast::<u32>().read_unaligned(), occupant as u32, &[occupant as u32]);
            let lot = result as u32 as *mut u8;
            let powered = if lot.is_null() { false } else {
                result = lot_call(lot, 0x108, lot.cast::<u32>().read_unaligned(), (result >> 32) as u32, &[]);
                if result as u8 != 0 { true } else {
                    result = lot_call(lot, 0x104, result as u32, lot.cast::<u32>().read_unaligned(), &[]);
                    result as u8 == 0
                }
            };
            (result & 0xFFFF_FFFF_0000_0000) | u64::from(powered)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006eac40::PORT,
    fc4p_006ead70::PORT,
    fc4p_006eae20::PORT,
    fc4p_006eafd0::PORT,
    fc4p_006eb020::PORT,
    fc4p_006eb070::PORT,
    fc4p_006eb280::PORT,
    fc4p_006eb350::PORT,
    fc4p_006eb3d0::PORT,
];

#[cfg(test)]
mod tests;
