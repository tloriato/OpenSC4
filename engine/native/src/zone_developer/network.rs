// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Network occupant lookup and the two highlight-tool queries.

use super::{registers, Port, ZoneDeveloper};
use crate::abi::{Conv, RawCall};
use core::ptr::{null_mut, read_unaligned, write_unaligned};

game_data! { static allocator: *mut u8 = 0x00B2_07E4; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_temporary(slot: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn clear_nodes(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3E60; }
game_fn! { fn get_network(this: *mut u8, x: i32, z: i32, flags: u32) -> u64 = "thiscall" 0x0072_FCA0; }

/// A virtual integer call, including the scratch registers that can survive to the caller.
pub(super) unsafe fn virtual_call(this: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> RawCall {
    unsafe {
        let mut call = RawCall::new(Conv::Thiscall, crate::abi::vslot(this, slot), this as u32, args);
        call.eax = eax;
        call.edx = edx;
        registers::call(&mut call);
        call
    }
}

/// Acquire the game's cached list allocator, dropping the temporary service reference after caching it.
pub(super) unsafe fn list_allocator(mut edx: u32) -> (*mut u8, u32) {
    unsafe {
        if read_unaligned(allocator()).is_null() {
            let mut temporary = null_mut();
            let service = framework();
            if !service.is_null() {
                vcall!(service, 0x14 => fn(u32, u32, *mut *mut u8) -> u32,
                    0x3AE4_BEA3, 0x3AE4_BEAB, &mut temporary);
            }
            write_unaligned(allocator(), temporary);
            let mut release = RawCall::new(
                Conv::Thiscall,
                crate::abi::target(0x0041_1870),
                &mut temporary as *mut *mut u8 as u32,
                &[],
            );
            release.edx = temporary as u32;
            crate::abi::call_raw(&mut release);
            edx = release.edx;
        }
        (read_unaligned(allocator()), edx)
    }
}

port! {
    /// Find the first network occupant matching any requested flag. Release each queried interface,
    /// including the match, then release the occupant list and its sentinel. Return a borrowed pointer.
    /// Each query stores its interface pointer in the caller's X argument word.
    /// The entry adapter restores ECX alongside EDX:EAX from the last allocator call.
    #[port(address = 0x0072_FCA0, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::GetNetwork", naked)]
    pub unsafe extern "C" fn fc4p_0072fca0() {
        core::arch::naked_asm!("sub esp, 4", "lea edx, [esp + 4]", "push esp",
            "call {body}", "mov ecx, [esp]", "add esp, 4", "ret 12", body = sym lookup_network);
    }
}

unsafe extern "fastcall" fn lookup_network(this: *mut u8, frame: *mut u32, out_ecx: *mut u32) -> u64 {
    unsafe {
        let x = frame.add(1).read() as i32;
        let z = frame.add(2).read() as i32;
        let flags = frame.add(3).read();
        let developer = ZoneDeveloper::from_ptr(this);
        let (allocation_service, _) = list_allocator(0);
        let header = vcall!(allocation_service, 0x0C => fn(u32) -> *mut u8, 12);
        write_unaligned(header.cast::<*mut u8>(), header);
        write_unaligned(header.add(4).cast::<*mut u8>(), header);
        let mut list = header;
        let cell = [x, z];
        vcall!(developer.occupant_manager(), 0xA4 => fn(*mut *mut u8, *const i32, *const i32, u32, u32),
            &mut list, cell.as_ptr(), cell.as_ptr(), 0xC772_BF98, u32::MAX);
        let mut node = read_unaligned(list.cast::<*mut u8>());
        let mut found = null_mut();
        while node != list {
            let occupant = read_unaligned(node.add(8).cast::<*mut u8>());
            let network_slot = frame.add(1).cast::<*mut u8>();
            write_unaligned(network_slot, null_mut());
            let queried = vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0xA821_EF94, network_slot);
            let network = read_unaligned(network_slot);
            let matches = queried as u8 != 0 && vcall!(network, 0x24 => fn(u32) -> u32, flags) as u8 != 0;
            if matches {
                found = network;
            }
            if !network.is_null() {
                vcall!(network, 8 => fn());
            }
            if matches {
                break;
            }
            node = read_unaligned(node.cast::<*mut u8>());
        }
        let manager = developer.occupant_manager();
        let slot = &mut list as *mut *mut u8 as u32;
        let result = virtual_call(manager, 0xC0, slot, read_unaligned(manager.cast::<u32>()), &[slot]);
        let mut clear = RawCall::new(Conv::Thiscall, crate::abi::target(0x0041_3E60), slot, &[]);
        clear.eax = result.eax;
        clear.edx = result.edx;
        registers::call(&mut clear);
        let (allocation_service, edx) = list_allocator(clear.edx);
        let result = virtual_call(
            allocation_service,
            0x10,
            read_unaligned(allocation_service.cast::<u32>()),
            edx,
            &[list as u32],
        );
        write_unaligned(out_ecx, result.ecx);
        (u64::from(result.edx) << 32) | u64::from(found as u32)
    }
}

port! {
    /// Test real networks first. In highlight mode the street bit takes precedence over the dirt bit:
    /// when both are set, the dirt tool is never asked. Keep the selected call's EDX and upper EAX bytes.
    /// The highlight tool writes its edge mask into the caller's flags argument word.
    #[port(address = 0x0072_FE10, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ExistsNetworkOfType", naked)]
    pub unsafe extern "thiscall" fn fc4p_0072fe10(_this: *mut u8, _x: i32, _z: i32, _flags: u32) -> u64 {
        core::arch::naked_asm!("sub esp, 4", "lea edx, [esp + 4]", "push esp",
            "call {body}", "mov ecx, [esp]", "add esp, 4", "ret 12", body = sym exists_network);
    }
}

unsafe extern "fastcall" fn exists_network(this: *mut u8, frame: *mut u32, out_ecx: *mut u32) -> u64 {
    unsafe {
        let x = frame.add(1).read();
        let z = frame.add(2).read();
        let flags = frame.add(3).read();
        let developer = ZoneDeveloper::from_ptr(this);
        let args = [x, z, flags];
        let mut result = RawCall::new(Conv::Thiscall, crate::abi::target(0x0072_FCA0), this as u32, &args);
        registers::call(&mut result);
        let eax = result.eax;
        let normalized = (u64::from(result.edx) << 32) | u64::from((eax & !0xFF) | u32::from(eax != 0));
        if eax != 0 {
            write_unaligned(out_ecx, result.ecx);
            return normalized;
        }
        write_unaligned(out_ecx, (result.ecx & !0xFF) | u32::from(developer.real_mode()));
        if developer.real_mode() != 0 { return normalized; }
        let (tool, flag) = if flags & 8 != 0 { (developer.street_tool(), 8) }
            else if flags & 0x800 != 0 { (developer.dirt_tool(), 0x800) }
            else { return normalized; };
        let mask = frame.add(3);
        let result = virtual_call(tool, 0x24, read_unaligned(tool.cast::<u32>()),
            mask as u32, &[x, z, flag, mask as u32]);
        write_unaligned(out_ecx, result.ecx);
        (u64::from(result.edx) << 32) | u64::from(result.eax)
    }
}

port! {
    /// Map sides 0..3 to edges 1,2,3,0 and all other sides to -1. After real networks, highlight mode
    /// asks street then dirt whenever either draft flag is set. The first draft decides using its edge byte.
    /// Invalid sides read byte three because x86 masks the shift count. Keep the original EDX result.
    #[port(address = 0x0072_FE80, owner = "zonedeveloper", name = "cSC4ZoneDeveloper::ExistsConnectionOfType")]
    pub unsafe extern "thiscall" fn fc4p_0072fe80(this: *mut u8, x: i32, z: i32, flags: u32, side: u32, kind: u32) -> u64 {
        unsafe {
            let developer = ZoneDeveloper::from_ptr(this);
            let edge = if side <= 3 { side.wrapping_add(1) & 3 } else { u32::MAX };
            let shift = edge.wrapping_mul(8);
            let result = get_network(this, x, z, flags);
            let mut eax = result as u32;
            let mut edx = (result >> 32) as u32;
            if eax != 0 {
                let network = eax as *mut u8;
                let result = virtual_call(network, 0xBC, eax, read_unaligned(network.cast::<u32>()), &[edge, kind]);
                (eax, edx) = (result.eax, result.edx);
                if eax as u8 != 0 { return (u64::from(edx) << 32) | u64::from((eax & !0xFF) | 1); }
            }
            eax = (eax & !0xFF) | u32::from(developer.real_mode());
            if eax as u8 != 0 || flags & 0x808 == 0 { return (u64::from(edx) << 32) | u64::from(eax & !0xFF); }
            let mut mask = flags;
            let tool = developer.street_tool();
            let result = virtual_call(tool, 0x24, &mut mask as *mut u32 as u32,
                read_unaligned(tool.cast::<u32>()), &[x as u32, z as u32, 8, &mut mask as *mut u32 as u32]);
            if result.eax as u8 != 0 {
                let shifted = mask.wrapping_shr(shift);
                return (u64::from(shifted) << 32) | u64::from((result.eax & !0xFF) | u32::from(shifted as u8 != 0));
            }
            let tool = developer.dirt_tool();
            let result = virtual_call(tool, 0x24, read_unaligned(tool.cast::<u32>()),
                &mut mask as *mut u32 as u32, &[x as u32, z as u32, 0x800, &mut mask as *mut u32 as u32]);
            eax = if result.eax as u8 != 0 {
                let shifted = mask.wrapping_shr(shift);
                (shifted & !0xFF) | u32::from(shifted as u8 != 0)
            } else { result.eax & !0xFF };
            (u64::from(result.edx) << 32) | u64::from(eax)
        }
    }
}

pub(super) const PORTS: &[Port] = &[fc4p_0072fca0::PORT, fc4p_0072fe10::PORT, fc4p_0072fe80::PORT];

#[cfg(test)]
mod tests;
