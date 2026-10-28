// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Repositioning connection markers after terrain edits.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;

game_fn! { fn marker_flag(holder: *mut u8, id: u32, out: *mut u8) -> u64 = "C" 0x005F_D390; }
game_fn! { fn clear_props(list: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_3E60; }
game_data! { static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { static prop_manager: *mut u8 = 0x00B4_3D10; }
game_data! { static occupant_manager: *mut u8 = 0x00B4_3D0C; }
game_data! { static prop_allocator: *mut u8 = 0x00B2_07E4; }

port! {
    /// Clamps connection cells to the city, tests the inclusive changed rectangle, and moves marker
    /// props to the cell centre and maximum surface height. Elevated transport adds 15.5 after the
    /// surface height is stored as a float. Every queried prop list is submitted, cleared and freed.
    #[port(address = 0x006D_0BE0, owner = "neighbors", name = "cSC4NeighborsSimulator::DoTerrainChanged")]
    pub unsafe extern "fastcall" fn fc4p_006d0be0(this: *mut u8, entry_edx: u32, rect: *const i32) -> u64 {
        // SAFETY: the game supplies the simulator, rectangle and live services and list objects.
        unsafe {
            let sim = NeighborsSim::from_ptr(this);
            let city = sim.city();
            let result = virtual_call(city, 0x174, &[], word(city, 0), entry_edx);
            let max_x = (result as u32 as i32).wrapping_sub(1);
            let city = sim.city();
            let result = virtual_call(city, 0x178, &[], result as u32, word(city, 0));
            let max_z = (result as u32 as i32).wrapping_sub(1);
            let header = sim.connections();
            let mut node = pointer(header, 0);
            let mut result = pair(header as u32, (result >> 32) as u32);
            while node != sim.connections() {
                let connection = pointer(node, 8);
                let mut cell = [0i32; 2];
                virtual_call(connection, 0x18,
                    &[cell.as_mut_ptr() as u32, cell.as_mut_ptr().add(1) as u32],
                    word(connection, 0), cell.as_mut_ptr() as u32);
                cell[0] = if cell[0] < 0 { 0 } else if cell[0] > max_x { max_x } else { cell[0] };
                cell[1] = if cell[1] < 0 { 0 } else if cell[1] > max_z { max_z } else { cell[1] };
                result = pair(cell[1] as u32, cell[0] as u32);
                if cell[0] >= rect.read_unaligned() && cell[1] >= rect.add(1).read_unaligned()
                    && cell[0] <= rect.add(2).read_unaligned() && cell[1] <= rect.add(3).read_unaligned() {
                    let mut position = [0u32; 3];
                    let city = sim.city();
                    virtual_call(city, 0x184,
                        &[cell[0] as u32, cell[1] as u32, position.as_mut_ptr() as u32, position.as_mut_ptr().add(2) as u32],
                        cell[1] as u32, cell[0] as u32);
                    let terrain = terrain().read();
                    let args = [cell[0] as u32, cell[1] as u32];
                    let mut call = RawCall::new(Conv::Thiscall, vslot(terrain, 0x48), terrain as u32, &args);
                    call.eax = word(terrain, 0); call.edx = cell[0] as u32; call.take_st0 = 1;
                    call_raw(&mut call);
                    position[1] = call.st0.to_f32().to_bits();
                    let kind_result = virtual_call(connection, 0x14, &[], word(connection, 0), call.edx);
                    if matches!(kind_result as u32, 3 | 7 | 11) {
                        position[1] = (F80::from_f32(f32::from_bits(position[1])) + F80::from_f32(15.5)).to_f32().to_bits();
                    }
                    let mut edx = (kind_result >> 32) as u32;
                    let mut scratch = core::ptr::null_mut();
                    let allocator = super::cities::service(prop_allocator(), &mut scratch, &mut edx);
                    let allocation = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
                    let mut props = allocation as u32 as *mut u8;
                    props.cast::<*mut u8>().write_unaligned(props); props.add(4).cast::<*mut u8>().write_unaligned(props);
                    let bounds = [cell[0], cell[1], cell[0], cell[1]];
                    let manager = prop_manager().read();
                    virtual_call(manager, 0x48,
                        &[&mut props as *mut *mut u8 as u32, bounds.as_ptr() as u32],
                        &mut props as *mut *mut u8 as u32, word(manager, 0));
                    let mut prop_node = pointer(props, 0);
                    while prop_node != props {
                        let prop = pointer(prop_node, 8);
                        let mut marker = 0u8;
                        let holder = virtual_call(prop, 0x18, &[], props as u32, word(prop, 0)) as u32 as *mut u8;
                        let marker_result = marker_flag(holder, 0xCAA4_6595, &mut marker);
                        if marker != 0 {
                            let result = virtual_call(prop, 0x24, &[position.as_mut_ptr() as u32], word(prop, 0), (marker_result >> 32) as u32);
                            let manager = occupant_manager().read();
                            virtual_call(manager, 0x60, &[prop as u32, 1], result as u32, word(manager, 0));
                        }
                        prop_node = pointer(prop_node, 0);
                    }
                    let manager = occupant_manager().read();
                    let submitted = virtual_call(manager, 0xC0, &[&mut props as *mut *mut u8 as u32], word(manager, 0), &mut props as *mut *mut u8 as u32);
                    let cleared = clear_props(&mut props, (submitted >> 32) as u32);
                    edx = (cleared >> 32) as u32;
                    let saved = props;
                    let allocator = super::cities::service(prop_allocator(), &mut scratch, &mut edx);
                    result = virtual_call(allocator, 0x10, &[saved as u32], word(allocator, 0), edx);
                }
                node = pointer(node, 0);
            }
            result
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006d0be0::PORT];

#[cfg(test)]
mod tests;
