// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Filtering and removal of network connection occupants.

use super::maintenance::OccupantNode;
use super::*;

game_data! { static node_allocator: *mut u8 = 0x00B2_07E4; }
game_fn! { #[allow(dead_code)] fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_service(service: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn network_service() -> u64 = "C" 0x0060_2680; }
game_fn! { #[allow(dead_code)] fn destroy_list(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3FE0; }

unsafe fn direct(address: u32, conv: Conv, this: u32, eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, &[]);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

/// Resolves the shared node allocator again after releasing the temporary service interface.
pub(super) unsafe fn allocator(mut call: RawCall) -> (*mut u8, RawCall) {
    unsafe {
        if node_allocator().read().is_null() {
            let mut service = core::ptr::null_mut::<u8>();
            call = direct(0x0087_93EC, Conv::Cdecl, 0, call.eax, call.edx);
            let root = call.eax as *mut u8;
            if !root.is_null() {
                call = virtual_call(
                    root,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, &mut service as *mut *mut u8 as u32],
                    call.eax,
                    table(root),
                    false,
                );
            }
            node_allocator().write(service);
            call = direct(
                0x0041_1870,
                Conv::Thiscall,
                &mut service as *mut *mut u8 as u32,
                call.eax,
                service as u32,
            );
        }
        (node_allocator().read(), call)
    }
}

/// Calls the final zero-argument destructor and captures its otherwise volatile ECX result.
unsafe fn destroy_with_registers(call: &mut RawCall) {
    unsafe {
        core::arch::asm!(
            "mov eax, dword ptr [edi + 4]", "mov ecx, dword ptr [edi + 8]", "mov edx, dword ptr [edi + 12]",
            "call dword ptr [edi]", "mov dword ptr [edi + 4], eax", "mov dword ptr [edi + 8], ecx",
            "mov dword ptr [edi + 12], edx", in("edi") call as *mut RawCall, clobber_abi("C"),
        );
    }
}

port! {
    /// Supplies the caller's lot word, reused as the temporary list, and retains all three result words.
    #[port(address = 0x006C_9FB0, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveNetworkConnectionNodes", naked)]
    pub unsafe extern "C" fn fc4p_006c9fb0() {
        core::arch::naked_asm!(
            "sub esp, 12", "lea eax, [esp + 16]", "mov edx, esp", "push eax", "call {}",
            "mov eax, [esp]", "mov edx, [esp + 4]", "mov ecx, [esp + 8]", "add esp, 12", "ret 4",
            sym remove_network_connections,
        );
    }
}

unsafe extern "fastcall" fn remove_network_connections(this: *mut u8, registers: *mut [u32; 3], list: *mut *mut u8) {
    unsafe {
        let lot = list.read_unaligned();
        let mut rectangle = [0u32; 4];
        let call = virtual_call(
            lot,
            0x38,
            &[rectangle.as_mut_ptr() as u32],
            table(lot),
            rectangle.as_mut_ptr() as u32,
            false,
        );
        list.write_unaligned(core::ptr::null_mut());
        let (allocation, call) = allocator(call);
        let mut call = virtual_call(allocation, 0x0C, &[12], table(allocation), call.edx, false);
        let head = call.eax as *mut u8;
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        list.write_unaligned(head);
        let min = [rectangle[0], rectangle[1]];
        let max = [rectangle[2], rectangle[3]];
        let developer = LotDeveloper::from_ptr(this);
        let manager = developer.occupant_manager();
        call = virtual_call(
            manager,
            0xA4,
            &[
                list as u32,
                min.as_ptr() as u32,
                max.as_ptr() as u32,
                0xC772_BF98,
                u32::MAX,
            ],
            list as u32,
            table(manager),
            false,
        );
        if call.eax as u8 != 0 {
            let sentinel = list.read_unaligned();
            let mut current = OccupantNode::from_ptr(sentinel).next();
            while current != sentinel {
                let occupant = OccupantNode::from_ptr(current).occupant();
                let mut interface = core::ptr::null_mut::<u8>();
                call = virtual_call(
                    occupant,
                    0,
                    &[0xA821_EF94, &mut interface as *mut *mut u8 as u32],
                    &mut interface as *mut *mut u8 as u32,
                    table(occupant),
                    false,
                );
                call = virtual_call(interface, 0x20, &[0x0020_0000], call.eax, table(interface), false);
                if call.eax as u8 == 0 {
                    call = virtual_call(occupant, 8, &[], table(occupant), call.edx, false);
                    let node = OccupantNode::from_ptr(current);
                    let next = node.next();
                    let previous = node.previous();
                    OccupantNode::from_ptr(previous).set_next(next);
                    OccupantNode::from_ptr(next).set_previous(previous);
                    let (allocation, returned) = allocator(call);
                    call = virtual_call(
                        allocation,
                        0x10,
                        &[current as u32],
                        table(allocation),
                        returned.edx,
                        false,
                    );
                    current = next;
                } else {
                    current = OccupantNode::from_ptr(current).next();
                }
                if !interface.is_null() {
                    call = virtual_call(interface, 8, &[], call.eax, table(interface), false);
                }
            }
            call = direct(0x0060_2680, Conv::Cdecl, 0, call.eax, call.edx);
            let service = call.eax as *mut u8;
            virtual_call(service, 0x18, &[list as u32], call.eax, table(service), false);
            let manager = developer.occupant_manager();
            call = virtual_call(manager, 0xC0, &[list as u32], list as u32, table(manager), false);
        }
        call.target = crate::abi::target(0x0041_3FE0);
        call.ecx = list as u32;
        call.count = 0;
        call.caller_pops = 0;
        destroy_with_registers(&mut call);
        registers.write_unaligned([call.eax, call.edx, call.ecx]);
    }
}

pub const PORTS: &[Port] = &[fc4p_006c9fb0::PORT];
