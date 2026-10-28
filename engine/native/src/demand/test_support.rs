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
    pub ecx: u32,
    pub extra_word: u32,
    pub popped: u32,
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
            ecx: 0,
            extra_word: 0,
            popped: 0,
        }
    }
    pub unsafe fn run(&mut self) {
        unsafe {
            probe(self);
        }
    }
    pub unsafe fn checked_run(&mut self, popped: u32) {
        environment(|| unsafe { self.run() });
        assert_eq!(self.popped, popped, "callee argument pop");
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
        "push dword ptr [edi + 44]",
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
        "mov dword ptr [edi + 40], ecx",
        "mov eax, esp",
        "sub eax, ebx",
        "mov dword ptr [edi + 48], eax",
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

/// Run a port with an existing x87 value and check the floating-point environment and direction flag.
pub(super) fn environment<T>(run: impl FnOnce() -> T) -> T {
    #[repr(C, align(16))]
    struct Fpu([u8; 512]);
    let mut before = Fpu([0; 512]);
    let mut after = Fpu([0; 512]);
    let (flags_before, flags_after): (u32, u32);
    // SAFETY: FXSAVE has aligned 512-byte buffers; FLD1's value is popped after the call.
    unsafe {
        core::arch::asm!("fld1", "fxsave [{p}]", "pushfd", "pop {flags}",
            p = in(reg) &mut before, flags = out(reg) flags_before, out("st(0)") _);
    }
    let result = run();
    unsafe {
        core::arch::asm!("fxsave [{p}]", "fstp st(0)", "pushfd", "pop {flags}",
            p = in(reg) &mut after, flags = out(reg) flags_after, out("st(0)") _);
    }
    assert_eq!(before.0[0..2], after.0[0..2], "x87 control word");
    assert_eq!(before.0[3] & 0x38, after.0[3] & 0x38, "x87 stack position");
    assert_eq!(before.0[4], after.0[4], "x87 occupied registers");
    assert_eq!(before.0[32..42], after.0[32..42], "existing x87 value");
    assert_eq!(before.0[24..28], after.0[24..28], "MXCSR");
    assert_eq!(flags_before & 0x400, flags_after & 0x400, "direction flag");
    result
}
