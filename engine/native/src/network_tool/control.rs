// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network tool reset, shutdown, and neighbor connection state.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};

game_fn! { fn erase_bytes(last: *mut u8, end: *mut u8, first: *mut u8, scratch: *mut u8, count: u32) -> u64 = "C" 0x0061_43E0; }
game_fn! { fn erase_neighbors(last: *mut u8, end: *mut u8, first: *mut u8, scratch: *mut u8, count: u32) -> u64 = "C" 0x0062_40D0; }
game_fn! { fn erase_anchors(last: *mut u8, end: *mut u8, first: *mut u8, scratch: *mut u8, count: u32) -> u64 = "C" 0x0062_4070; }
game_fn! { fn clear_world(this: *mut u8) = "thiscall" 0x0064_7B20; }
game_fn! { fn find_cell(this: *mut u8, key: u32) -> *mut u8 = "thiscall" 0x0063_3220; }
game_fn! { fn update_cell(this: *mut u8) = "thiscall" 0x0064_7830; }
game_fn! { #[allow(dead_code)] fn shutdown_preview(this: *mut u8) -> u64 = "thiscall" 0x0061_FBC0; }
game_data! { static messages: *mut u8 = 0x00B4_3CCC; }
game_data! { static preview_manager: *mut u8 = 0x00B4_3DD0; }

port! {
    /// Erase the entire cell-record vector through its shared range helper and retain its returned end.
    #[port(address = 0x0062_6130, owner = "networktool", name = "ClearCellRecordVector")]
    pub unsafe extern "thiscall" fn fc4p_00626130(this: *mut u8) -> u64 {
        unsafe {
            let mut scratch = core::mem::MaybeUninit::<u8>::uninit();
            let end = read(this, 4);
            let result = erase_bytes(end, end, read(this, 0), scratch.as_mut_ptr(), 0);
            write(this, 4, result as u32);
            result
        }
    }
}

port! {
    /// Promote each pending one-bit edge connection to two bits if any existing edge is two, merge
    /// the four edges into the cached cell, update the cell, and erase the pending records.
    #[port(address = 0x0062_6EB0, owner = "networktool", name = "cSC4NetworkTool::MarkNeighborConnections")]
    pub unsafe extern "thiscall" fn fc4p_00626eb0(this: *mut u8) -> u64 {
        unsafe {
            let tool = NetworkTool::from_ptr(this);
            let mut p: *mut u8 = read(this, 0x100);
            while p != read::<*mut u8>(this, 0x104) {
                let cell = find_cell(this, read(p, 0));
                let flags_offset = 0x18usize.wrapping_add(tool.network_type().wrapping_mul(4) as usize);
                let flags: u32 = read(cell, flags_offset);
                if [24, 16, 8, 0].into_iter().any(|shift| (flags >> shift) & 3 == 2) {
                    let mut pending: u32 = read(p, 4);
                    for shift in [0, 8, 16, 24] {
                        let bit = 1u32 << shift;
                        if pending & bit != 0 { pending = (pending & !bit) | bit.wrapping_mul(2); }
                    }
                    write(p, 4, pending);
                }
                let offset = 0x18usize.wrapping_add(tool.network_type().wrapping_mul(4) as usize);
                write(cell, offset, flags | read::<u32>(p, 4));
                update_cell(cell);
                p = p.wrapping_add(0x18);
            }
            let mut scratch = core::mem::MaybeUninit::<u8>::uninit();
            let begin = read(this, 0x100);
            let end = read(this, 0x104);
            let result = erase_neighbors(end, end, begin, scratch.as_mut_ptr(), 0);
            write(this, 0x104, result as u32);
            result
        }
    }
}

port! {
    /// Clear the world cache and previews, empty the retained pair/byte vectors, restore the current
    /// selection sentinels, clear failure through the virtual slot, and erase the anchor records.
    #[port(address = 0x0062_6F90, owner = "networktool", name = "cSC4NetworkTool::Reset")]
    pub unsafe extern "thiscall" fn fc4p_00626f90(this: *mut u8) -> u64 {
        unsafe {
            clear_world(this.add(0x1C));
            vcall!(this, 0x44 => fn());
            vcall!(this, 0x40 => fn());
            write(this, 0x11C, read::<u32>(this, 0x118));
            write(this, 0x64, read::<u32>(this, 0x60));
            write(this, 0x200, u32::MAX);
            write(this, 0x204, 0xBF80_0000u32);
            write(this, 0x1F8, read::<u32>(this, 0x1F4));
            vcall!(this, 0x34 => fn());
            let begin = read(this, 0xF0);
            let end = read(this, 0xF4);
            let mut scratch = core::mem::MaybeUninit::<u8>::uninit();
            let result = erase_anchors(end, end, begin, scratch.as_mut_ptr(), 0);
            write(this, 0xF4, result as u32);
            result
        }
    }
}

register_port!(0x0062_6D30, fc4p_00626d30, "cSC4NetworkTool::Shutdown", shutdown, "ret");

// Success replaces AL alone; inactive objects retain the caller's upper EAX bytes and EDX.
unsafe extern "fastcall" fn shutdown(this: *mut u8, entry: *mut u32) -> u64 {
    unsafe {
        let eax = *entry.add(1);
        let edx = *entry;
        if NetworkTool::from_ptr(this).initialized() == 0 {
            return registers((eax & !0xFF) | 1, edx);
        }
        shutdown_active(this, edx)
    }
}

unsafe fn invoke(object: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> (u32, u32) {
    unsafe {
        let mut c = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
        c.eax = eax;
        c.edx = edx;
        call_raw(&mut c);
        (c.eax, c.edx)
    }
}

unsafe fn shutdown_active(this: *mut u8, mut edx: u32) -> u64 {
    unsafe {
        let tool = NetworkTool::from_ptr(this);
        let server = *messages();
        if !server.is_null() {
            edx = invoke(
                server,
                0x18,
                read(server, 0),
                this.add(4) as u32,
                &[this.add(4) as u32, 0xA6B7_9602],
            )
            .1;
        }
        write(this, 0x1F8, read::<u32>(this, 0x1F4));
        invoke(this, 0x44, read(this, 0), edx, &[]);
        write(this, 0x11C, read::<u32>(this, 0x118));
        let old_end: u32 = read(this, 0x64);
        write(this, 0x64, read::<u32>(this, 0x60));
        let (mut eax, mut edx) = invoke(this, 0x40, old_end, read(this, 0), &[]);
        for offset in [0x4C, 0x48, 0x44, 0x364] {
            let p: *mut u8 = read(this, offset);
            if !p.is_null() {
                if offset == 0x4C || offset == 0x44 {
                    eax = read(p, 0);
                } else {
                    edx = read(p, 0);
                }
                (eax, edx) = invoke(p, 8, eax, edx, &[]);
                write(this, offset, 0u32);
            }
        }
        eax = tool.preview() as u32;
        if eax != 0 {
            let manager = *preview_manager();
            (eax, edx) = invoke(manager, 0x84, eax, read(manager, 0), &[eax]);
            let mut c = RawCall::new(
                Conv::Thiscall,
                crate::abi::target(0x0061_FBC0),
                tool.preview() as u32,
                &[],
            );
            c.eax = eax;
            c.edx = edx;
            call_raw(&mut c);
            eax = c.eax;
            edx = c.edx;
            let p = tool.preview();
            if !p.is_null() {
                tool.set_preview(core::ptr::null_mut());
                (eax, edx) = invoke(p, 8, read(p, 0), edx, &[]);
            }
        }
        tool.set_initialized(0);
        registers((eax & !0xFF) | 1, edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00626130::PORT,
    fc4p_00626d30::PORT,
    fc4p_00626eb0::PORT,
    fc4p_00626f90::PORT,
];

#[cfg(test)]
mod tests;
