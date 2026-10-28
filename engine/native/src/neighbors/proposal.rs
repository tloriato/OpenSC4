// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Single-deal proposals using the game's own constructor.

use super::{virtual_call, word};
use crate::port::Port;

game_fn! {
    fn allocate(bytes: u32) -> *mut u8 = "C" 0x005E_55E0;
}
game_fn! {
    fn construct_deal(this: *mut u8) -> *mut u8 = "thiscall" 0x006C_F270;
}
game_data! {
    static region_manager: *mut u8 = 0x00B4_3C94;
}

port! {
    /// Allocates a deal before looking for its connection. A successful proposal copies the two
    /// cities' region positions, sets type and zero amount, and publishes a retained reference.
    /// The coordinate getters reuse the original type and city argument words.
    #[port(address = 0x006C_F780, owner = "neighbors", name = "cSC4NeighborsSimulator::ProposeDeal", naked)]
    pub unsafe extern "C" fn fc4p_006cf780() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "call {}", "ret 12", sym propose);
    }
}

unsafe extern "fastcall" fn propose(this: *mut u8, argument: *mut u32) -> u64 {
    unsafe {
        let kind = argument.read_unaligned();
        let city = argument.add(1).read_unaligned() as *mut u8;
        let out = argument.add(2).read_unaligned() as *mut *mut u8;
        let storage = allocate(0x80);
        let deal = if storage.is_null() {
            storage
        } else {
            construct_deal(storage)
        };
        if !deal.is_null() {
            virtual_call(deal, 4, &[], word(deal, 0), 0);
        }
        let result = virtual_call(this, 0x5C, &[kind, city as u32], city as u32, word(this, 0));
        let connection = result as u32 as *mut u8;
        if connection.is_null() {
            let result = if deal.is_null() {
                result
            } else {
                virtual_call(deal, 8, &[], result as u32, word(deal, 0))
            };
            return result & !0xFF;
        }
        virtual_call(
            connection,
            0x20,
            &[argument as u32, argument.add(1) as u32],
            connection as u32,
            word(connection, 0),
        );
        let result = virtual_call(
            deal,
            0x14,
            &[argument.read_unaligned(), argument.add(1).read_unaligned()],
            argument.add(1).read_unaligned(),
            word(deal, 0),
        );
        let manager = region_manager().read();
        let result = virtual_call(manager, 0x90, &[], result as u32, word(manager, 0));
        let regional_city = result as u32 as *mut u8;
        virtual_call(
            regional_city,
            0x14,
            &[argument as u32, argument.add(1) as u32],
            regional_city as u32,
            word(regional_city, 0),
        );
        let result = virtual_call(
            deal,
            0x0C,
            &[argument.read_unaligned(), argument.add(1).read_unaligned()],
            argument.add(1).read_unaligned(),
            word(deal, 0),
        );
        virtual_call(deal, 0x24, &[kind], result as u32, word(deal, 0));
        let result = virtual_call(deal, 0x2C, &[0], word(deal, 0), 0);
        out.write_unaligned(deal);
        let result = virtual_call(deal, 4, &[], result as u32, word(deal, 0));
        let result = virtual_call(deal, 8, &[], word(deal, 0), (result >> 32) as u32);
        (result & !0xFF) | 1
    }
}

pub const PORTS: &[Port] = &[fc4p_006cf780::PORT];
