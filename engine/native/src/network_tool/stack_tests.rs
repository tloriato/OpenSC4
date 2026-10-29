// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Invoke an entry while recording its exact stack address for callee pointer checks.

#[repr(C)]
pub(super) struct StackCall {
    target: usize,
    this: *mut u8,
    args: *const u32,
    count: usize,
    pub entry: usize,
    eax: u32,
    edx: u32,
}

impl StackCall {
    pub fn new(target: usize, this: *mut u8, args: &[u32]) -> Self {
        Self {
            target,
            this,
            args: args.as_ptr(),
            count: args.len(),
            entry: 0,
            eax: 0,
            edx: 0,
        }
    }

    pub unsafe fn run(&mut self) -> (u32, u32) {
        unsafe { invoke(self) };
        (self.eax, self.edx)
    }
}

#[unsafe(naked)]
unsafe extern "thiscall" fn invoke(call: *mut StackCall) {
    core::arch::naked_asm!(
        "push ebx",
        "push esi",
        "push edi",
        "mov edi, ecx",
        "mov ebx, esp",
        "mov ecx, [edi + 12]",
        "mov esi, [edi + 8]",
        "2:",
        "test ecx, ecx",
        "jz 3f",
        "dec ecx",
        "push dword ptr [esi + ecx*4]",
        "jmp 2b",
        "3:",
        "lea eax, [esp - 4]",
        "mov [edi + 16], eax",
        "mov ecx, [edi + 4]",
        "call dword ptr [edi]",
        "mov [edi + 20], eax",
        "mov [edi + 24], edx",
        "mov esp, ebx",
        "pop edi",
        "pop esi",
        "pop ebx",
        "ret",
    );
}
