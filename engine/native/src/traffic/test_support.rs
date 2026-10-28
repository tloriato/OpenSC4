// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

use super::*;
use crate::abi::{Conv, RawCall};

pub unsafe fn invoke(function: usize, this: *mut u8, args: &[u32], eax: u32, edx: u32, cdecl: bool) -> Registers {
    let mut call = RawCall::new(
        if cdecl { Conv::Cdecl } else { Conv::Thiscall },
        function,
        this as u32,
        args,
    );
    call.eax = eax;
    call.edx = edx;
    unsafe {
        core::arch::asm!("mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call dword ptr [edi]",
            "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
            in("edi") &mut call, clobber_abi("C"));
    }
    Registers {
        eax: call.eax,
        ecx: call.ecx,
        edx: call.edx,
    }
}

pub fn table(slots: &[(usize, usize)]) -> Vec<usize> {
    let mut v = vec![0; 128];
    for &(slot, f) in slots {
        v[slot / 4] = f;
    }
    v
}

pub fn object(size: usize, vtable: &[usize]) -> crate::testing::Object {
    let mut o = crate::testing::Object::new(size);
    unsafe {
        core::ptr::write_bytes(o.ptr(), 0xA5, size);
        store(o.ptr(), 0, vtable.as_ptr() as u32);
    }
    o
}

pub unsafe fn bytes(p: *mut u8, size: usize) -> Vec<u8> {
    unsafe { std::slice::from_raw_parts(p, size).to_vec() }
}

pub fn float_environment() -> (u16, u16, u32, u32) {
    let (mut cw, mut mxcsr) = (0u16, 0u32);
    let (sw, flags): (u16, u32);
    unsafe {
        core::arch::asm!("fnstcw word ptr [{cw}]", "fnstsw ax", "stmxcsr dword ptr [{mxcsr}]",
            "pushfd", "pop {flags:e}", cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr,
            flags = lateout(reg) flags, out("ax") sw);
    }
    (cw, sw & 0x3800, mxcsr, flags & 0x400)
}
