// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Removal of occupants selected by a cell filter.

use super::*;
use core::mem::MaybeUninit;

game_data! { static list_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static demolition: *mut u8 = 0x00B4_3D00; }
game_fn! { fn allocator_factory() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_holder(this: *mut *mut u8) = "thiscall" 0x0041_1870; }
game_fn! { fn interface_output(this: *mut *mut u8) -> *mut *mut u8 = "thiscall" 0x004F_20A0; }
game_fn! { fn destroy_list(this: *mut *mut u8) -> u64 = "thiscall" 0x0041_3FE0; }

register_port!(
    0x0062_72F0,
    fc4p_006272f0,
    "cSC4NetworkTool::ClearNetworkOccsFromCell",
    clear,
    "ret 12"
);

unsafe extern "fastcall" fn clear(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let x = *frame.add(3);
        let z = *frame.add(4);
        let filter = *frame.add(5) as *mut u8;
        if filter.is_null() {
            return registers(*frame.add(1), *frame);
        }
        if (*list_allocator()).is_null() {
            let mut allocator = core::ptr::null_mut();
            let factory = allocator_factory();
            if !factory.is_null() {
                vcall!(factory, 0x14 => fn(u32, u32, *mut *mut u8), 0x3AE4_BEA3, 0x3AE4_BEAB, &mut allocator);
            }
            *list_allocator() = allocator;
            release_holder(&mut allocator);
        }
        let head = vcall!(*list_allocator(), 0xC => fn(u32) -> *mut u8, 12);
        write(head, 0, head);
        write(head, 4, head);
        let mut list = head;
        let cell = [x, z];
        let tool = NetworkTool::from_ptr(this);
        let success = vcall!(tool.occupant_manager(), 0xA0 => fn(*mut *mut u8, *const u32, *const u32, u32, u32) -> u32,
            &mut list, cell.as_ptr(), cell.as_ptr(), 0, u32::MAX);
        if success as u8 != 0 {
            let mut node: *mut u8 = read(list, 0);
            while node != list {
                let occupant: *mut u8 = read(node, 8);
                if vcall!(filter, 0xC => fn(*mut u8) -> u32, occupant) as u8 != 0 {
                    vcall!(tool.occupant_manager(), 0x54 => fn(*mut u8, u32, u32), occupant, 1, 0);
                } else if ![5, 7, 4].contains(&tool.network_type())
                    && vcall!(occupant, 0x1C => fn() -> u32) == 0x2890_D4DE
                    && vcall!(occupant, 0x80 => fn(u32) -> u32, 1) as u8 != 0
                {
                    let mut interface = core::ptr::null_mut();
                    let output = interface_output(&mut interface);
                    if vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0xA8C8_6142, output) as u8 != 0 {
                        let mut position = [MaybeUninit::<u32>::uninit(); 2];
                        vcall!(interface, 0x20 => fn(*mut u32, *mut u32), position[0].as_mut_ptr(), position[1].as_mut_ptr());
                        if position[0].assume_init() == x && position[1].assume_init() == z {
                            vcall!(*demolition(), 0x20 => fn(u32, *mut u8, u32, u32, u32, u32, u32),
                                1, occupant, 1, 0x2001, 0, 0, 0);
                        }
                    }
                    if !interface.is_null() {
                        vcall!(interface, 8 => fn());
                    }
                }
                node = read(node, 0);
            }
            vcall!(tool.occupant_manager(), 0xC0 => fn(*mut *mut u8), &mut list);
        }
        destroy_list(&mut list)
    }
}

pub const PORTS: &[Port] = &[fc4p_006272f0::PORT];

#[cfg(test)]
mod tests;
