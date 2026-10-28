// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Destruction of the shared network type information record.

use super::*;
use crate::abi::{Conv, RawCall};

game_fn! { #[allow(dead_code)] fn clear_buckets(this: *mut u8) -> u64 = "thiscall" 0x008E_BA60; }
game_fn! { #[allow(dead_code)] fn free_storage(p: *mut u8) -> u64 = "C" 0x0090_CF63; }

// This shared destructor also exposes the ECX left by its last helper. Keep it in the entry frame
// while the Rust body returns EDX:EAX, then restore it in the ABI wrapper.
port! {
    /// Destroy bucket tables and vector storage in reverse member order. Pointers remain stored;
    /// only the nested vector's vtable is restored. Preserve the last helper's ECX and EDX.
    #[port(address = 0x0062_6160, owner = "networktool", name = "DestroyNetworkTypeInfo", naked)]
    pub unsafe extern "C" fn fc4p_00626160() {
        core::arch::naked_asm!(
            "push eax", "push edx", "push ecx", "mov edx, esp", "call {body}",
            "pop ecx", "add esp, 8", "ret", body = sym destroy,
        )
    }
}

// call_raw's call contract, additionally retaining ECX for this helper's register result.
unsafe fn call_with_ecx(c: &mut RawCall) {
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
            "4:", in("edi") c as *mut RawCall, clobber_abi("C"),
        );
    }
}

unsafe fn clear(c: &mut RawCall, this: *mut u8) {
    c.target = crate::abi::target(0x008E_BA60);
    c.ecx = this as u32;
    c.count = 0;
    c.caller_pops = 0;
    unsafe {
        call_with_ecx(c);
    }
}

unsafe fn free(c: &mut RawCall, pointer: u32) {
    if pointer != 0 {
        c.target = crate::abi::target(0x0090_CF63);
        let args = [pointer];
        c.args = args.as_ptr();
        c.count = 1;
        c.caller_pops = 1;
        unsafe {
            call_with_ecx(c);
        }
    }
}

unsafe extern "fastcall" fn destroy(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let mut c = RawCall::new(Conv::Thiscall, 0, this as u32, &[]);
        c.eax = *frame.add(2);
        c.edx = *frame.add(1);
        clear(&mut c, this.add(0xD0));
        free(&mut c, read(this, 0xD4));
        for offset in [0xB8, 0x9C] {
            c.eax = read(this, offset);
            let p = c.eax;
            free(&mut c, p);
        }
        write(this, 0x80, 0x00A8_0810u32);
        c.eax = read(this, 0x84);
        let p = c.eax;
        free(&mut c, p);
        clear(&mut c, this.add(0x64));
        free(&mut c, read(this, 0x68));
        for offset in [0x58, 0x4C, 0x34, 0x28] {
            c.eax = read(this, offset);
            let p = c.eax;
            free(&mut c, p);
        }
        free(&mut c, read(this, 4));
        *frame = c.ecx;
        registers(c.eax, c.edx)
    }
}

pub const PORTS: &[Port] = &[fc4p_00626160::PORT];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::fake;
    use std::cell::RefCell;

    thread_local! { static LOG: RefCell<Vec<(u32, usize)>> = const { RefCell::new(Vec::new()) }; }
    extern "thiscall" fn clear_impl(this: *mut u8) -> u64 {
        LOG.with(|s| s.borrow_mut().push((0, this as usize)));
        registers(0xAABB_CCDD, 0xEEFF_0011)
    }
    extern "C" fn free_impl(pointer: u32) -> u64 {
        LOG.with(|s| s.borrow_mut().push((1, pointer as usize)));
        registers(0x1234_5678, 0x9ABC_DEF0)
    }
    #[unsafe(naked)]
    unsafe extern "C" fn free_fake() {
        core::arch::naked_asm!(
            "push dword ptr [esp + 4]", "call {body}", "add esp, 4", "mov ecx, 0x24681357", "ret",
            body = sym free_impl,
        )
    }
    #[unsafe(naked)]
    unsafe extern "C" fn clear_fake() {
        core::arch::naked_asm!("call {body}", "mov ecx, 0x13572468", "ret", body = sym clear_impl)
    }

    #[test]
    fn type_info_destruction_orders_members_keeps_pointers_and_returns_helper_registers() {
        let _a = fake(0x008E_BA60, clear_fake as *const () as usize);
        let _b = fake(0x0090_CF63, free_fake as *const () as usize);
        for bitmap in [0u32, 1, 0x155, 0x2AA, 0x3FF] {
            let mut storage = [0xA5A5_A5A5u32; 0xE8 / 4];
            let offsets = [0xD4, 0xB8, 0x9C, 0x84, 0x68, 0x58, 0x4C, 0x34, 0x28, 4];
            for (index, offset) in offsets.into_iter().enumerate() {
                storage[offset / 4] = if bitmap & (1 << index) != 0 {
                    0x1000 + offset as u32
                } else {
                    0
                };
            }
            let t = storage.as_mut_ptr().cast::<u8>();
            let mut expected = storage;
            expected[0x80 / 4] = 0x00A8_0810;
            LOG.with(|s| s.borrow_mut().clear());
            let mut c = RawCall::new(Conv::Thiscall, fc4p_00626160 as *const () as usize, t as u32, &[]);
            c.eax = 0x8765_4321;
            c.edx = 0x5555_AAAA;
            unsafe {
                call_with_ecx(&mut c);
            }
            assert_eq!(storage, expected);
            let mut events = vec![(0, t as usize + 0xD0)];
            for (index, offset) in offsets.into_iter().enumerate() {
                if index == 4 {
                    events.push((0, t as usize + 0x64));
                }
                if bitmap & (1 << index) != 0 {
                    events.push((1, 0x1000 + offset));
                }
            }
            LOG.with(|s| assert_eq!(*s.borrow(), events));
            assert_eq!(c.edx, if bitmap & 0x3F0 != 0 { 0x9ABC_DEF0 } else { 0xEEFF_0011 });
            assert_eq!(c.ecx, if bitmap & 0x3F0 != 0 { 0x2468_1357 } else { 0x1357_2468 });
            assert_eq!(c.eax, if bitmap & 0x300 != 0 { 0x1234_5678 } else { 0 });
        }
    }
}
