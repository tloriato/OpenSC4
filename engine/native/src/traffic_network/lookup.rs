// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Tile lookups and the map's small interface operations.

use super::{table_word, virtual_words, words, TileInfo, TrafficNetworkMap};
use crate::port::Port;
use crate::x87::F80;

game_data! { static cell_scale: f32 = 0x00A8_FDF0; }

port! {
    /// Selects prop occupants with either the 0x5002 or 0x5003 flag. AL is normalized;
    /// the remaining bytes come from the last virtual call.
    #[port(address = 0x004A_DF00, owner = "trafficnetwork", name = "is traffic intersection prop", naked)]
    pub unsafe extern "C" fn fc4p_004adf00() {
        core::arch::naked_asm!("jmp {}", sym prop_body)
    }
}

unsafe extern "fastcall" fn prop_body(_ecx: u32, entry_edx: u32, occupant: *mut u8) -> u64 {
    unsafe {
        let mut result = virtual_words(occupant, 0x1C, &[], table_word(occupant), entry_edx);
        if result as u32 != 0xA823_821E {
            return result & !0xFF;
        }
        let vt = table_word(occupant);
        result = virtual_words(occupant, 0x58, &[0x5002], result as u32, vt);
        if result as u8 == 0 {
            result = virtual_words(occupant, 0x58, &[0x5003], table_word(occupant), (result >> 32) as u32);
        }
        (result & !0xFF) | u64::from(result as u8 != 0)
    }
}

port! {
    /// Tests each tile's network in chain order, stopping when slot 0x70 succeeds.
    #[port(address = 0x004A_E030, owner = "trafficnetwork", name = "test networks at cell")]
    pub unsafe extern "thiscall" fn fc4p_004ae030(
        this: *mut u8, x: i32, z: i32, a: u32, b: u32, c: u32
    ) -> u64 {
        unsafe {
            let mut result = virtual_words(this, 0x38, &[x as u32, z as u32], table_word(this), x as u32);
            let mut tile = result as u32 as *mut u8;
            while !tile.is_null() {
                let network = TileInfo::from_ptr(tile).network() as u32;
                result = virtual_words(this, 0x70, &[network, a, b, c], table_word(this), network);
                if result as u8 != 0 {
                    return (result & !0xFF) | 1;
                }
                tile = TileInfo::from_ptr(tile).next();
            }
            result
        }
    }
}

port! {
    /// Passes the caller's EAX to the Rust body; the four stack words and their cleanup are unchanged.
    #[port(address = 0x004A_E090, owner = "trafficnetwork", name = "test network connection", naked)]
    pub unsafe extern "C" fn fc4p_004ae090() {
        core::arch::naked_asm!("mov ecx, edx", "mov edx, eax", "jmp {}", sym connection_body)
    }
}

// ECX is unused by this stdcall entry, so it can carry entry EDX while EDX carries entry EAX.
unsafe extern "fastcall" fn connection_body(
    entry_edx: u32,
    entry_eax: u32,
    network: *mut u8,
    a: u32,
    b: u32,
    c: u32,
) -> u64 {
    if network.is_null() {
        return words(entry_eax & !0xFF, entry_edx);
    }
    unsafe {
        let result = virtual_words(network, 0xD4, &[c, a, b], table_word(network), c);
        (result & !0xFF) | u64::from(result as u8 != 0)
    }
}

port! {
    /// Reads a cell only when both signed coordinates lie inside the map.
    #[port(address = 0x004A_E0C0, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::SafeGetTileInfoAtCell", naked)]
    pub unsafe extern "C" fn fc4p_004ae0c0() {
        core::arch::naked_asm!("jmp {}", sym safe_cell_body)
    }
}

unsafe extern "fastcall" fn safe_cell_body(this: *mut u8, entry_edx: u32, x: i32, z: i32) -> u64 {
    unsafe {
        let map = TrafficNetworkMap::from_ptr(this);
        if x < 0 || x >= map.width() {
            return words(0, entry_edx);
        }
        if z < 0 || z >= map.height() {
            return words(0, z as u32);
        }
        virtual_words(this, 0x34, &[x as u32, z as u32], table_word(this), z as u32)
    }
}

port! {
    /// Supplies the original argument word's address as the second coordinate output.
    #[port(address = 0x004A_E0F0, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::GetTileInfoByNetwork", naked)]
    pub unsafe extern "C" fn fc4p_004ae0f0() {
        core::arch::naked_asm!("lea edx, [esp + 4]", "jmp {}", sym by_network_body)
    }
}

unsafe extern "fastcall" fn by_network_body(this: *mut u8, argument: *mut i32, network: *mut u8) -> u64 {
    unsafe {
        let mut x = this as i32;
        vcall!(network, 0xEC => fn(*mut i32, *mut i32), &mut x, argument);
        let result = virtual_words(
            this,
            0x38,
            &[x as u32, argument.read() as u32],
            table_word(this),
            x as u32,
        );
        let mut tile = result as u32 as *mut u8;
        while !tile.is_null() && TileInfo::from_ptr(tile).network() != network {
            tile = TileInfo::from_ptr(tile).next();
        }
        words(tile as u32, (result >> 32) as u32)
    }
}

port! {
    /// Returns the first tile's network, or null when the cell has no tile.
    #[port(address = 0x004A_E140, owner = "trafficnetwork", name = "get network at cell")]
    pub unsafe extern "thiscall" fn fc4p_004ae140(this: *mut u8, x: i32, z: i32) -> u64 {
        unsafe {
            let result = virtual_words(this, 0x38, &[x as u32, z as u32], table_word(this), x as u32);
            let tile = result as u32 as *mut u8;
            let network = if tile.is_null() { tile } else { TileInfo::from_ptr(tile).network() };
            words(network as u32, (result >> 32) as u32)
        }
    }
}

port! {
    /// Converts world Z then world X through the game's truncation helper and forwards the cell query.
    #[port(address = 0x004A_E190, owner = "trafficnetwork", name = "query network at world point")]
    pub unsafe extern "thiscall" fn fc4p_004ae190(
        this: *mut u8, point: *const f32, a: u32, b: u32
    ) -> u64 {
        unsafe {
            // Keep the method selected before the two conversions, as the caller's map does.
            let target = crate::abi::vslot(this, 0x20);
            let z = (F80::from_f32(point.add(2).read()) * F80::from_f32(cell_scale().read())).ftol() as u32;
            let x = (F80::from_f32(point.read()) * F80::from_f32(cell_scale().read())).ftol() as u64;
            let args = [x as u32, z, a, b];
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, target, this as u32, &args);
            call.eax = x as u32;
            call.edx = (x >> 32) as u32;
            crate::abi::call_raw(&mut call);
            words(call.eax, call.edx)
        }
    }
}

port! {
    /// Answers the primary, unknown, message-target and serializable interfaces; successful queries AddRef.
    /// Unrecognized IDs leave the output untouched and clear only AL of the ID arithmetic result.
    #[port(address = 0x004A_E350, owner = "trafficnetwork", name = "cSC4TrafficNetworkMap::QueryInterface")]
    pub unsafe extern "thiscall" fn fc4p_004ae350(this: *mut u8, iid: u32, out: *mut *mut u8) -> u32 {
        unsafe {
            let offset = match iid {
                0xCA0F_81B9 => 0,
                1 => 4,
                0x4522_94AA => 8,
                0xE4FD_A3D4 => 12,
                _ => {
                    let residue = if iid < 0xCA0F_81B9 {
                        iid.wrapping_sub(0x4522_94AA)
                    } else { iid };
                    return residue & !0xFF;
                }
            };
            out.write(if this.is_null() { this } else { this.wrapping_add(offset) });
            let result = vcall!(this, 4 => fn() -> u32);
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Finds a matching first word among 16-byte records, returning the end pointer on failure.
    #[port(address = 0x004A_E460, owner = "trafficnetwork", name = "find traffic record", naked)]
    pub unsafe extern "C" fn fc4p_004ae460() {
        core::arch::naked_asm!("mov ecx, esp", "jmp {}", sym find_body)
    }
}

unsafe extern "fastcall" fn find_body(frame: *const u32, entry_edx: u32) -> u64 {
    unsafe {
        let mut begin = frame.add(1).read() as *mut u8;
        let end = frame.add(2).read() as *mut u8;
        let key = frame.add(3).read() as *const u32;
        let mut edx = entry_edx;
        let groups = (end as i32).wrapping_sub(begin as i32) >> 6;
        if groups > 0 {
            let sought = key.read_unaligned();
            edx = sought;
            for _ in 0..groups {
                for _ in 0..4 {
                    if begin.cast::<u32>().read_unaligned() == sought {
                        return words(begin as u32, edx);
                    }
                    begin = begin.wrapping_add(16);
                }
            }
        }
        let tail = (end as i32).wrapping_sub(begin as i32) >> 4;
        if (1..=3).contains(&tail) {
            for index in 0..tail {
                let record = begin.cast::<u32>().read_unaligned();
                if tail - index == 2 {
                    edx = record;
                }
                if record == key.read_unaligned() {
                    return words(begin as u32, edx);
                }
                begin = begin.wrapping_add(16);
            }
        }
        words(end as u32, edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_004adf00::PORT,
    fc4p_004ae030::PORT,
    fc4p_004ae090::PORT,
    fc4p_004ae0c0::PORT,
    fc4p_004ae0f0::PORT,
    fc4p_004ae140::PORT,
    fc4p_004ae190::PORT,
    fc4p_004ae350::PORT,
    fc4p_004ae460::PORT,
];

#[cfg(test)]
mod tests;
