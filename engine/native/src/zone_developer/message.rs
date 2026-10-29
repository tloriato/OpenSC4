// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Messages arriving through the zone developer's message interface.

use super::{registers, Port};
use crate::abi::{Conv, RawCall};
use core::{
    mem::MaybeUninit,
    ptr::{null_mut, read_unaligned, write_unaligned},
};

layout! {
    /// Fields relative to the message interface, eight bytes after the primary interface.
    struct MessageDeveloper {
        0x09 => enabled / set_enabled: u8,
        0x1C => city / set_city: *mut u8,
        0x20 => cells_x / set_cells_x: i32,
        0x24 => cells_z / set_cells_z: i32,
        0x30 => renderer / set_renderer: *mut u8,
        0x38 => zone_manager / set_zone_manager: *mut u8,
        0x3C => dirt_tool / set_dirt_tool: *mut u8,
        0x40 => street_tool / set_street_tool: *mut u8,
        0x44 => road_tool / set_road_tool: *mut u8,
        0x4C => lot_rows / set_lot_rows: *mut u8,
        0x98 => current_zone / set_current_zone: u32,
        0xCC => zoning_setting / set_zoning_setting: u32,
    }
}

game_fn! { #[allow(dead_code)] fn renderer() -> *mut u8 = "C" 0x0060_2160; }
game_fn! { #[allow(dead_code)] fn tool_registry() -> *mut u8 = "C" 0x0060_2680; }
game_fn! { #[allow(dead_code)] fn plop_all_lot_types(this: *mut u8) -> u64 = "thiscall" 0x0072_C8E0; }
game_fn! { #[allow(dead_code)] fn plop_all_building_types(this: *mut u8) -> u64 = "thiscall" 0x0072_BE50; }
game_fn! { #[allow(dead_code)] fn extend_streets(this: *mut u8, occupant: *mut u8) -> u64 = "thiscall" 0x0073_1F60; }
game_fn! { #[allow(dead_code)] fn cross_rail(this: *mut u8, occupant: *mut u8) -> u64 = "thiscall" 0x0073_2450; }
game_fn! {
    #[allow(dead_code)]
    fn create_lot(this: *mut u8, x: i32, z: i32, width: i32, height: i32, facing: u32) -> u64
        = "thiscall" 0x0072_D440;
}

/// Track the scratch registers across game calls, including getters returning only a byte.
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

impl Registers {
    unsafe fn invoke(&mut self, address: usize, convention: Conv, args: &[u32]) {
        let mut call = RawCall::new(convention, address, self.ecx, args);
        call.eax = self.eax;
        call.edx = self.edx;
        unsafe { registers::call(&mut call) };
        self.eax = call.eax;
        self.ecx = call.ecx;
        self.edx = call.edx;
    }

    unsafe fn virtual_call(&mut self, this: *mut u8, slot: usize, table_in_eax: bool, args: &[u32]) {
        let table = unsafe { read_unaligned(this.cast::<u32>()) };
        if table_in_eax {
            self.eax = table;
        } else {
            self.edx = table;
        }
        self.ecx = this as u32;
        unsafe { self.invoke(crate::abi::vslot(this, slot), Conv::Thiscall, args) };
    }

    unsafe fn game_call(&mut self, address: u32, this: Option<*mut u8>, args: &[u32]) {
        let convention = if let Some(this) = this {
            self.ecx = this as u32;
            Conv::Thiscall
        } else {
            Conv::Cdecl
        };
        unsafe { self.invoke(crate::abi::target(address), convention, args) };
    }

    fn byte(&mut self, value: u8) {
        self.eax = (self.eax & !0xFF) | u32::from(value);
    }

    fn result(mut self) -> u64 {
        self.byte(1);
        (u64::from(self.edx) << 32) | u64::from(self.eax)
    }
}

port! {
    /// Dispatch initialization, plop commands, occupant changes and zoning settings. The argument slot is also a cell output
    /// and the initialization scan's counter, so pass its original address to the Rust body.
    #[port(address = 0x0073_28D0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_007328d0() {
        core::arch::naked_asm!("lea eax, [esp + 4]", "push eax", "call {body}", "ret 4", body = sym do_message);
    }
}

unsafe extern "fastcall" fn do_message(this: *mut u8, entry_edx: u32, argument: *mut u32) -> u64 {
    unsafe {
        let dev = MessageDeveloper::from_ptr(this);
        let message = read_unaligned(argument) as *mut u8;
        let mut r = Registers {
            eax: 0,
            ecx: this as u32,
            edx: entry_edx,
        };
        r.virtual_call(message, 0x10, true, &[]);
        match r.eax {
            0x26D3_1EC1 => initialize(dev, &mut r, argument),
            0x230E_27AC => {
                r.virtual_call(message, 0x24, true, &[]);
                if r.eax == 0x8929_E68E {
                    r.game_call(0x0072_C8E0, Some(this.sub(8)), &[]);
                } else {
                    r.virtual_call(message, 0x24, false, &[]);
                    if r.eax == 0x4AB5_69C9 {
                        r.game_call(0x0072_BE50, Some(this.sub(8)), &[]);
                    }
                }
            }
            0x99EF_1142 => occupant_changed(dev, message, argument, &mut r),
            0xA6B7_9602 => {
                r.virtual_call(message, 0x3C, false, &[]);
                r.eax = read_unaligned(r.eax as *const u32);
                dev.set_zoning_setting(r.eax);
            }
            _ => {}
        }
        r.result()
    }
}

unsafe fn initialize(dev: MessageDeveloper, r: &mut Registers, argument: *mut u32) {
    unsafe {
        r.eax = dev.renderer() as u32;
        if r.eax == 0 {
            r.game_call(0x0060_2160, None, &[]);
            dev.set_renderer(r.eax as *mut u8);
            r.virtual_call(r.eax as *mut u8, 4, false, &[]);
        }
        for (offset, kind) in [(0x40, 3), (0x3C, 11), (0x44, 0)] {
            let field = dev.as_ptr().add(offset).cast::<*mut u8>();
            if read_unaligned(field).is_null() {
                r.game_call(0x0060_2680, None, &[]);
                r.virtual_call(r.eax as *mut u8, 0x14, false, &[kind, 1]);
                write_unaligned(field, r.eax as *mut u8);
                r.virtual_call(r.eax as *mut u8, 4, false, &[]);
                r.virtual_call(read_unaligned(field), 0x0C, true, &[]);
            }
        }
        r.byte(dev.enabled());
        if dev.enabled() == 0 {
            return;
        }
        write_unaligned(argument, 0);
        if dev.cells_x() <= 0 {
            return;
        }
        let mut row_offset = 0u32;
        loop {
            r.eax = dev.cells_z() as u32;
            let mut z = 0i32;
            if z < r.eax as i32 {
                loop {
                    r.ecx = dev.lot_rows() as u32;
                    r.edx = read_unaligned(dev.lot_rows().wrapping_add(row_offset as usize).cast::<u32>());
                    let lot = read_unaligned((r.edx as *const *mut u8).wrapping_offset(z as isize));
                    if !lot.is_null() {
                        let mut origin = MaybeUninit::<[i32; 2]>::uninit();
                        let x_out = origin.as_mut_ptr().cast::<i32>();
                        let z_out = x_out.add(1);
                        r.edx = x_out as u32;
                        r.virtual_call(lot, 0x54, true, &[x_out as u32, z_out as u32]);
                        r.eax = read_unaligned(argument);
                        let origin = origin.assume_init();
                        if r.eax as i32 == origin[0] && z == origin[1] {
                            r.virtual_call(lot, 0x20, false, &[]);
                        }
                    }
                    r.eax = dev.cells_z() as u32;
                    z = z.wrapping_add(1);
                    if z >= r.eax as i32 {
                        break;
                    }
                }
            }
            r.eax = read_unaligned(argument).wrapping_add(1);
            r.ecx = dev.cells_x() as u32;
            row_offset = row_offset.wrapping_add(12);
            write_unaligned(argument, r.eax);
            if r.eax as i32 >= r.ecx as i32 {
                break;
            }
        }
    }
}

unsafe fn occupant_changed(dev: MessageDeveloper, message: *mut u8, argument: *mut u32, r: &mut Registers) {
    unsafe {
        r.virtual_call(message, 0x4C, true, &[]);
        if r.eax != 0xC772_BF98 {
            return;
        }
        r.virtual_call(message, 0x28, false, &[]);
        let occupant = r.eax as *mut u8;
        let mut network = null_mut::<u8>();
        r.virtual_call(occupant, 0, true, &[0xA821_EF94, &mut network as *mut *mut u8 as u32]);
        if r.eax as u8 != 0 {
            r.virtual_call(network, 0x24, false, &[0x8020_0090]);
            let skip = r.eax as u8 != 0;
            r.virtual_call(network, 8, true, &[]);
            r.byte(u8::from(!skip));
            if skip {
                return;
            }
        }
        let mut position = MaybeUninit::<[u32; 3]>::uninit();
        r.eax = position.as_mut_ptr() as u32;
        r.virtual_call(occupant, 0x20, false, &[r.eax]);
        let position = position.assume_init();
        let mut z = MaybeUninit::<i32>::uninit();
        r.eax = position[0];
        r.virtual_call(
            dev.city(),
            0x17C,
            false,
            &[position[0], position[2], argument as u32, z.as_mut_ptr() as u32],
        );
        r.eax = read_unaligned(argument);
        r.edx = dev.lot_rows() as u32;
        r.ecx = r.eax.wrapping_mul(3);
        r.eax = read_unaligned(
            dev.lot_rows()
                .wrapping_add(r.ecx.wrapping_mul(4) as usize)
                .cast::<u32>(),
        );
        r.ecx = z.assume_init() as u32;
        let lot = read_unaligned((r.eax as *const *mut u8).wrapping_offset(r.ecx as i32 as isize));
        if !lot.is_null() {
            let mut bounds = MaybeUninit::<[i32; 4]>::uninit();
            r.eax = bounds.as_mut_ptr() as u32;
            r.virtual_call(lot, 0x38, false, &[r.eax]);
            r.virtual_call(lot, 0x5C, false, &[]);
            let facing = r.eax;
            r.virtual_call(lot, 0x7C, true, &[]);
            dev.set_current_zone(r.eax);
            r.virtual_call(dev.as_ptr().sub(4), 0x60, false, &[lot as u32]);
            r.edx = dev.current_zone();
            r.virtual_call(dev.zone_manager(), 0x34, true, &[r.edx]);
            if r.eax as u8 != 0 {
                let [x0, z0, x1, z1] = bounds.assume_init();
                r.edx = x1.wrapping_sub(x0).wrapping_add(1) as u32;
                r.ecx = x0 as u32;
                r.eax = z0 as u32;
                r.game_call(
                    0x0072_D440,
                    Some(dev.as_ptr().sub(8)),
                    &[
                        x0 as u32,
                        z0 as u32,
                        r.edx,
                        z1.wrapping_sub(z0).wrapping_add(1) as u32,
                        facing,
                    ],
                );
            }
            dev.set_current_zone(0);
        }
        r.virtual_call(message, 0x60, true, &[]);
        match r.eax {
            1 | 2 => r.game_call(0x0073_1F60, Some(dev.as_ptr().sub(8)), &[occupant as u32]),
            3 => r.game_call(0x0073_2450, Some(dev.as_ptr().sub(8)), &[occupant as u32]),
            _ => {}
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_007328d0::PORT];

#[cfg(test)]
mod tests;
