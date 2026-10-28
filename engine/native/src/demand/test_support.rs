// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Observe caller-owned argument words immediately after the callee returns.

#[repr(C)]
pub(super) struct Probe {
    pub target: usize,
    pub this: *mut u8,
    pub args: [u32; 3],
    pub after: [u32; 3],
    pub eax: u32,
    pub edx: u32,
}

impl Probe {
    pub fn new(target: *const (), this: *mut u8, args: [u32; 3]) -> Self {
        Self {
            target: target as usize,
            this,
            args,
            after: [0; 3],
            eax: 0x1122_3344,
            edx: 0x5566_7788,
        }
    }
    pub unsafe fn run(&mut self) {
        unsafe {
            probe(self);
        }
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn probe(_context: *mut Probe) {
    // The caller always supplies three words of backing storage. Each tested port pops its own arguments;
    // EBP restores the probe's stack independently of that pop, before restoring the saved registers.
    core::arch::naked_asm!(
        "push ebp",
        "push ebx",
        "push esi",
        "push edi",
        "mov ebp, esp",
        "mov edi, ecx",
        "push dword ptr [edi + 16]",
        "push dword ptr [edi + 12]",
        "push dword ptr [edi + 8]",
        "mov ebx, esp",
        "mov ecx, dword ptr [edi + 4]",
        "mov eax, dword ptr [edi + 32]",
        "mov edx, dword ptr [edi + 36]",
        "call dword ptr [edi]",
        "mov dword ptr [edi + 32], eax",
        "mov dword ptr [edi + 36], edx",
        "mov eax, dword ptr [ebx]",
        "mov dword ptr [edi + 20], eax",
        "mov eax, dword ptr [ebx + 4]",
        "mov dword ptr [edi + 24], eax",
        "mov eax, dword ptr [ebx + 8]",
        "mov dword ptr [edi + 28], eax",
        "mov esp, ebp",
        "pop edi",
        "pop esi",
        "pop ebx",
        "pop ebp",
        "ret",
    )
}
