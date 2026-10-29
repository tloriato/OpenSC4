// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Registering listeners immediately or deferring registration during a nested update.

use super::*;
use core::mem::MaybeUninit;

layout! {
    /// Listener registry, with its lock, nesting counter, pending queue and keyed listener lists.
    pub struct ListenerRegistry {
        0x64 => nesting / set_nesting: u32,
        0x68 => pending / set_pending: u32,
        0x6C => listeners / set_listeners: u32,
    }
}

game_fn! { #[allow(dead_code)] fn lock(this: u32, object: u32) -> u32 = "thiscall" 0x0048_C9F0; }
game_fn! { #[allow(dead_code)] fn retain(this: u32, object: u32) -> u32 = "thiscall" 0x004F_20C0; }
game_fn! { #[allow(dead_code)] fn copy_registration(this: u32, source: u32) -> u32 = "thiscall" 0x0092_D1E3; }
game_fn! { #[allow(dead_code)] fn defer(this: u32, registration: u32) -> u32 = "thiscall" 0x0092_E11A; }
game_fn! { #[allow(dead_code)] fn find(this: u32, key: u32) -> u32 = "thiscall" 0x0058_EFE0; }
game_fn! { #[allow(dead_code)] fn allocate(bytes: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_list(this: u32, allocator: u32) -> u32 = "thiscall" 0x004F_5A00; }
game_fn! { #[allow(dead_code)] fn insert(this: u32, result: u32, pair: u32) -> u32 = "thiscall" 0x0092_FDD8; }
game_fn! { #[allow(dead_code)] fn append(this: u32, value: u32) -> u32 = "thiscall" 0x007E_98C0; }
game_fn! { #[allow(dead_code)] fn assign(this: u32, object: u32) -> u32 = "thiscall" 0x0054_47B0; }
game_fn! { #[allow(dead_code)] fn flush(this: u32) -> u32 = "thiscall" 0x0092_DF35; }

register_port! {
    /// Register an object under a key, ignoring duplicates. Nested updates queue a retained copy.
    /// Immediate updates decrement nesting and flush pending work before releasing the lock.
    /// Only AL is replaced by the success byte; the final Release supplies the other registers.
    0x0092_E795, fc4p_0092e795, register_listener, 2, 8, "RegisterKeyedListener", |r| {
        let this = r.ecx;
        let registry = ListenerRegistry::from_ptr(this as *mut u8);
        let mut local = MaybeUninit::<[u32; 7]>::uninit();
        let local = local.as_mut_ptr() as u32;
        store(local, 0x18, 0);
        r.eax = this.wrapping_add(0x20);
        let lock_object = r.eax;
        r.ecx = local.wrapping_add(0xC);
        invoke(r, Conv::Thiscall, crate::abi::target(0x0048_C9F0), &[lock_object]);
        if registry.nesting() != 0 {
            r.ecx = local.wrapping_add(0x14);
            invoke(r, Conv::Thiscall, crate::abi::target(0x004F_20C0), &[r.args[0]]);
            r.eax = r.args[1]; store(local, 0x18, r.eax);
            r.eax = local.wrapping_add(0x14);
            let pair = r.eax;
            r.ecx = local.wrapping_add(4);
            write_unaligned(local as *mut u8, 1);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0092_D1E3), &[pair]);
            r.eax = local;
            r.ecx = this.wrapping_add(0x68);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0092_E11A), &[r.eax]);
            for offset in [4, 0x14] {
                r.ecx = word(local, offset);
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
                }
            }
        } else {
            r.eax = &mut r.args[1] as *mut u32 as u32;
            r.ecx = this.wrapping_add(0x6C);
            registry.set_nesting(1);
            invoke(r, Conv::Thiscall, crate::abi::target(0x0058_EFE0), &[r.eax]);
            let list;
            let mut duplicate = false;
            if r.eax == 0 {
                invoke(r, Conv::Cdecl, crate::abi::target(0x005E_55E0), &[4]);
                list = r.eax;
                // The caller removes the allocation argument with POP ECX.
                r.ecx = 4;
                if list != 0 {
                    r.eax = (&mut r.args[1] as *mut u32 as u32).wrapping_add(3);
                    r.ecx = list;
                    invoke(r, Conv::Thiscall, crate::abi::target(0x004F_5A00), &[r.eax]);
                }
                r.eax = r.args[1]; store(local, 0x14, r.eax);
                r.eax = local.wrapping_add(0x14);
                let pair = r.eax;
                r.eax = local; r.ecx = this.wrapping_add(0x6C);
                store(local, 0x18, list);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0092_FDD8), &[r.eax, pair]);
            } else {
                list = word(r.eax, 8);
                r.ecx = word(list, 0); r.eax = word(r.ecx, 0);
                while r.eax != r.ecx {
                    r.edx = r.args[0];
                    if r.edx == word(r.eax, 8) { duplicate = true; break; }
                    r.eax = word(r.eax, 0);
                }
            }
            if !duplicate {
                r.args[1] = 0;
                r.eax = &mut r.args[1] as *mut u32 as u32;
                r.ecx = list;
                invoke(r, Conv::Thiscall, crate::abi::target(0x007E_98C0), &[r.eax]);
                r.ecx = r.args[1];
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
                }
                let header = word(list, 0);
                r.ecx = word(header, 4).wrapping_add(8);
                invoke(r, Conv::Thiscall, crate::abi::target(0x0054_47B0), &[r.args[0]]);
            }
            registry.set_nesting(registry.nesting().wrapping_sub(1));
            r.ecx = this;
            invoke(r, Conv::Thiscall, crate::abi::target(0x0092_DF35), &[]);
        }
        r.ecx = word(local, 0x10); r.eax = word(r.ecx, 0);
        invoke(r, Conv::Thiscall, word(r.eax, 8) as usize, &[]);
        r.eax = (r.eax & !0xFF) | 1;
    }
}

pub const PORTS: &[Port] = &[fc4p_0092e795::PORT];

#[cfg(test)]
mod tests;
