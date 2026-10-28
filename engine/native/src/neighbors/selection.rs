// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection selection for regional commodity deals.

use super::{pair, pointer, virtual_call, word};
use crate::port::Port;

game_fn! {
    fn framework() -> u64 = "C" 0x0087_93EC;
}
game_fn! {
    fn release_local(local: *mut *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0041_1870;
}
game_fn! {
    fn destroy_connections(list: *mut *mut u8, entry_edx: u32) -> u64 = "fastcall" 0x0041_1F80;
}
game_fn! {
    fn erase_connection(list: *mut *mut u8, entry_edx: u32, out: *mut u32, node: *mut u8) -> u64 = "fastcall" 0x0041_C000;
}
game_data! {
    static connection_allocator: *mut u8 = 0x00B2_07DC;
}

unsafe fn empty_list(entry_edx: u32) -> (*mut u8, u32) {
    // SAFETY: the allocator service and its cache belong to the game.
    unsafe {
        let mut allocator = connection_allocator().read();
        let mut edx = entry_edx;
        if allocator.is_null() {
            let mut local = core::ptr::null_mut();
            let result = framework();
            let framework = result as u32 as *mut u8;
            if !framework.is_null() {
                virtual_call(framework, 0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, &mut local as *mut *mut u8 as u32],
                    framework as u32, word(framework, 0));
            }
            connection_allocator().write(local);
            let result = release_local(&mut local, local as u32);
            edx = (result >> 32) as u32;
            allocator = connection_allocator().read();
        }
        let result = virtual_call(allocator, 0x0C, &[12], word(allocator, 0), edx);
        let header = result as u32 as *mut u8;
        header.cast::<*mut u8>().write_unaligned(header);
        header.add(4).cast::<*mut u8>().write_unaligned(header);
        (header, (result >> 32) as u32)
    }
}

port! {
    /// Searches utility connections for activity on the required side, or uses the first available
    /// transport type for garbage. Rejected utility entries are erased from the temporary list.
    /// The erase helper writes its next iterator into the incoming city argument word.
    #[port(address = 0x006D_1FC0, owner = "neighbors", name = "cSC4NeighborsSimulator::FindConnectionSuitableForDeal", naked)]
    pub unsafe extern "C" fn fc4p_006d1fc0() {
        core::arch::naked_asm!("push edx", "lea edx, [esp + 8]", "call {}", "ret 8", sym suitable);
    }
}

unsafe extern "fastcall" fn suitable(this: *mut u8, args: *mut u32, entry_edx: u32) -> u64 {
    // SAFETY: the entry supplies live argument words, simulator and connection interfaces.
    unsafe {
        let (mut header, mut edx) = empty_list(entry_edx);
        let kind = args.read_unaligned();
        let city = args.add(1).read_unaligned();
        let mut check_activity = true;
        let sell = kind & 1 != 0;
        match kind {
            0x4773_FFE0..=0x4773_FFE3 => {
                let connection_kind = if kind < 0x4773_FFE2 { 4 } else { 5 };
                let result = virtual_call(this, 0x48,
                    &[connection_kind, city, &mut header as *mut *mut u8 as u32],
                    &mut header as *mut *mut u8 as u32, word(this, 0));
                edx = (result >> 32) as u32;
            }
            0x4773_FFE4 | 0x4773_FFE5 => {
                check_activity = false;
                for connection_kind in [3, 12, 8, 2, 10] {
                    let result = virtual_call(this, 0x48,
                        &[connection_kind, city, &mut header as *mut *mut u8 as u32],
                        &mut header as *mut *mut u8 as u32, word(this, 0));
                    edx = (result >> 32) as u32;
                    if pointer(header, 0) != header { break; }
                }
            }
            _ => {}
        }
        let mut found = core::ptr::null_mut();
        let mut node = pointer(header, 0);
        if node != header {
            if check_activity {
                let mut eax = (header as u32 & !0xFF) | 1;
                loop {
                    let connection = pointer(node, 8);
                    let mut result = virtual_call(this, 0x34, &[connection as u32], eax, word(this, 0));
                    let other = result as u32 as *mut u8;
                    let active = if sell {
                        result = virtual_call(connection, 0x34, &[], word(connection, 0), (result >> 32) as u32);
                        result as u8 != 0
                    } else if !other.is_null() {
                        result = virtual_call(other, 0x34, &[], other as u32, word(other, 0));
                        result as u8 != 0
                    } else {
                        false
                    };
                    edx = (result >> 32) as u32;
                    if active { found = connection; break; }
                    let result = erase_connection(&mut header, pointer(header, 0) as u32, args.add(1), pointer(header, 0));
                    edx = (result >> 32) as u32;
                    node = pointer(header, 0);
                    eax = header as u32;
                    if node == header {
                        break;
                    }
                }
            } else {
                found = pointer(node, 8);
            }
        }
        let result = destroy_connections(&mut header, edx);
        pair(found as u32, (result >> 32) as u32)
    }
}

pub const PORTS: &[Port] = &[fc4p_006d1fc0::PORT];

#[cfg(test)]
mod tests;
