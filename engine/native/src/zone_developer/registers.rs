// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Integer calls whose callers retain ECX as well as EDX:EAX.

use crate::abi::RawCall;

/// Like the integer part of `call_raw`, also recording ECX before the stack cleanup.
pub(super) unsafe fn call(call: &mut RawCall) {
    // SAFETY: the caller supplies a live game function with the convention and arguments in RawCall.
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:", in("edi") call as *mut RawCall, clobber_abi("C"));
    }
}
