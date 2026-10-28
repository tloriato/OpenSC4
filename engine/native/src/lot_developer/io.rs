// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Version-one output, including the queue's live cursor distance.

use super::*;

game_fn! { fn write_building(stream: *mut u8, building: *mut u8) -> u64 = "C" 0x005D_99A0; }
game_fn! { fn write_boolean(stream: *mut u8, value: u32) -> u64 = "C" 0x005D_9FD0; }

port! {
    /// Writes geometry, the building references, the cursor distance and two bytes. The serializable
    /// interface is eight bytes after the primary interface. Only the final error byte decides success.
    #[port(address = 0x006C_66F0, owner = "lotdeveloper", name = "cSC4LotDeveloper::Write")]
    pub unsafe extern "fastcall" fn fc4p_006c66f0(this: *mut u8, entry_edx: u32, stream: *mut u8) -> u64 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this.wrapping_sub(8));
            virtual_call(stream, 0x1C, &[1], table(stream), entry_edx, false);
            let vertices = developer.vertices_x() as u32;
            virtual_call(stream, 0x20, &[vertices], vertices, table(stream), false);
            let extent = developer.extent_x_bits();
            virtual_call(stream, 0x30, &[extent], extent, table(stream), false);
            let extent = developer.extent_z_bits();
            virtual_call(stream, 0x30, &[extent], extent, table(stream), false);
            let count = queue::distance(developer.night_begin(), developer.night_end()) as u32;
            virtual_call(stream, 0x24, &[count], count, table(stream), false);
            let mut current = developer.night_begin();
            let end = developer.night_end();
            while current.current != end.current {
                let building = current.current.read_unaligned() as *mut u8;
                write_building(stream, building);
                current.advance();
            }
            let cursor = queue::distance(developer.night_begin(), developer.night_cursor()) as u32;
            virtual_call(stream, 0x24, &[cursor], cursor, table(stream), false);
            let mode = u32::from(developer.night_mode());
            virtual_call(stream, 0x14, &[mode], mode, table(stream), false);
            let returned = write_boolean(stream, u32::from(developer.lit()));
            let mut call = virtual_call(stream, 0x48, &[], returned as u32, table(stream), false);
            call.eax = u32::from(call.eax as u8 == 0);
            result(call)
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c66f0::PORT];

#[cfg(test)]
mod tests;
