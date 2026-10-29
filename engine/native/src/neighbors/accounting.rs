// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Deal totals and the version-two connection record.

use super::{pointer, virtual_call, word, NeighborsSim};
use crate::port::Port;

game_fn! {
    #[cfg(test)]
    fn write_reference(stream: *mut u8, object: *mut u8) -> u64 = "C" 0x005D_99A0;
}

#[cfg(not(test))]
static WRITE_REFERENCE: usize = 0x005D_99A0;
#[cfg(test)]
static WRITE_REFERENCE: unsafe extern "C" fn(*mut u8, *mut u8) -> u64 = test_write_reference;
#[cfg(test)]
unsafe extern "C" fn test_write_reference(stream: *mut u8, object: *mut u8) -> u64 {
    unsafe { write_reference(stream, object) }
}

unsafe fn total(this: *mut u8, entry_edx: u32, first_type: u32) -> u64 {
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        let header = sim.deals();
        let mut node = pointer(header, 0);
        let mut sum = 0u64;
        let mut edx = entry_edx;
        while node != sim.deals() {
            let deal = pointer(node, 8);
            let flags = virtual_call(deal, 0x9C, &[0x3D], word(deal, 0), edx);
            edx = (flags >> 32) as u32;
            if flags as u8 == 0 {
                let kind = virtual_call(deal, 0x20, &[], flags as u32, word(deal, 0));
                edx = (kind >> 32) as u32;
                let kind = (kind as u32).wrapping_sub(first_type);
                if kind == 0 || kind == 2 || kind == 4 {
                    let cost = virtual_call(deal, 0x58, &[], word(deal, 0), edx);
                    edx = (cost >> 32) as u32;
                    sum = sum.wrapping_add(cost);
                }
            }
            node = pointer(node, 0);
        }
        sum
    }
}

port! {
    /// Sums sell-power, sell-water and sell-garbage prices with wrapping 64-bit addition, omitting
    /// deals with any flag in 0x3D. Active need not be set and expiration does not exclude a deal.
    #[port(address = 0x006C_F860, owner = "neighbors", name = "cSC4NeighborsSimulator::GetTotalMonthlyNeighborDealIncome")]
    pub unsafe extern "fastcall" fn fc4p_006cf860(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe { total(this, entry_edx, 0x4773_FFE1) }
    }
}

port! {
    /// Sums buy-power, buy-water and buy-garbage prices with the same flag test and wrapping addition.
    #[port(address = 0x006C_F8D0, owner = "neighbors", name = "cSC4NeighborsSimulator::GetTotalMonthlyNeighborDealExpense")]
    pub unsafe extern "fastcall" fn fc4p_006cf8d0(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe { total(this, entry_edx, 0x4773_FFE0) }
    }
}

port! {
    /// Writes version two, the connection count, and each serializable connection reference. Only
    /// the low byte of the final stream error decides success; intermediate failures do not stop it.
    /// `this` is the serializable interface, eight bytes beyond the primary interface.
    #[port(address = 0x006C_FB60, owner = "neighbors", name = "cSC4NeighborsSimulator::Write", naked)]
    pub unsafe extern "fastcall" fn fc4p_006cfb60(this: *mut u8, entry_edx: u32, stream: *mut u8) -> u64 {
        // The stream retains addresses of its callees' stack buffers in heap state.
        // Preserve the three saved words and call depth as well as the record data.
        core::arch::naked_asm!(
            "push ebx", "push esi", "push edi", "mov edi, [esp + 16]",
            "mov eax, [edi]", "mov ebx, ecx", "push 2", "mov ecx, edi",
            "call dword ptr [eax + 0x1c]", "mov ecx, [ebx + 0x14]", "mov eax, [ecx]",
            "xor edx, edx", "cmp eax, ecx", "je 3f",
            "2:", "mov eax, [eax]", "inc edx", "cmp eax, ecx", "jne 2b",
            "3:", "mov eax, [edi]", "push edx", "mov ecx, edi", "call dword ptr [eax + 0x24]",
            "mov eax, [ebx + 0x14]", "mov esi, [eax]", "cmp esi, eax", "je 5f",
            "4:", "mov ecx, [esi + 8]", "push ecx", "push edi", "call dword ptr [{reference}]",
            "mov esi, [esi]", "mov eax, [ebx + 0x14]", "add esp, 8", "cmp esi, eax", "jne 4b",
            "5:", "mov edx, [edi]", "mov ecx, edi", "call dword ptr [edx + 0x48]",
            "neg al", "pop edi", "pop esi", "pop ebx", "sbb eax, eax", "inc eax", "ret 4",
            reference = sym WRITE_REFERENCE,
        );
    }
}

pub const PORTS: &[Port] = &[fc4p_006cf860::PORT, fc4p_006cf8d0::PORT, fc4p_006cfb60::PORT];
