// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Preview construction, retained service, and the two rule trees.

use super::*;

layout! {
    /// `cSC4NetworkPreview`, including its two rule-tree headers and counts.
    struct Preview {
        0x00 => primary_vtable / set_primary_vtable: u32,
        0x04 => secondary_vtable / set_secondary_vtable: u32,
        0x0C => service / set_service: *mut u8,
        0x10 => first_tree / set_first_tree: *mut u8,
        0x14 => first_count / set_first_count: u32,
        0x1C => second_tree / set_second_tree: *mut u8,
        0x20 => second_count / set_second_count: u32,
        0x54 => initialized / set_initialized: u8,
    }
}

game_fn! { fn erase_first(this: *mut u8, root: *mut u8) = "thiscall" 0x0061_FCD0; }
game_fn! { fn erase_second(this: *mut u8, root: *mut u8) = "thiscall" 0x0062_03A0; }
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free(p: *mut u8) = "C" 0x0090_CF63; }
game_fn! { fn construct_interface(this: *mut u8) = "thiscall" 0x0090_DA1E; }
game_fn! { fn destroy_interface(this: *mut u8) -> u64 = "thiscall" 0x0090_D990; }
game_fn! { fn destroy_preview(this: *mut u8) -> u64 = "thiscall" 0x0062_0770; }
game_fn! { fn delete_storage(this: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { fn delete_preview(this: *mut u8, flags: u32) -> u64 = "thiscall" 0x0062_0BA0; }

register_port!(
    0x0061_FBC0,
    fc4p_0061fbc0,
    "ShutdownPreviewService",
    shutdown_service,
    "ret"
);
unsafe extern "fastcall" fn shutdown_service(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let p = Preview::from_ptr(this);
        let mut result = registers((frame.add(1).read() & !0xFF) | u32::from(p.initialized()), frame.read());
        if p.initialized() != 0 {
            let service = p.service();
            if !service.is_null() {
                p.set_service(core::ptr::null_mut());
                result = vcall!(service, 8 => fn() -> u64);
            }
            p.set_initialized(0);
        }
        result
    }
}

// Reset each header field after the erase callback; callbacks may replace the tree header.
unsafe fn clear_tree(this: *mut u8, second: bool) -> u32 {
    unsafe {
        let count: u32 = read(this, 4);
        if count == 0 {
            return 0;
        }
        let head: *mut u8 = read(this, 0);
        let root = read(head, 4);
        if second {
            erase_second(this, root);
        } else {
            erase_first(this, root);
        }
        let head: *mut u8 = read(this, 0);
        write(head, 8, head);
        let head: *mut u8 = read(this, 0);
        write(head, 4, 0u32);
        let last: *mut u8 = read(this, 0);
        write(last, 0xC, last);
        write(this, 4, 0u32);
        head as u32
    }
}
port! {
    /// Destroy the second tree before the first, freeing each header; release the retained service and
    /// destroy the secondary interface. Do not clear pointers that destruction no longer uses.
    #[port(address = 0x0062_0770, owner = "networktool", name = "DestroyNetworkPreview")]
    pub unsafe extern "thiscall" fn fc4p_00620770(this: *mut u8) -> u64 {
        unsafe {
            let p = Preview::from_ptr(this);
            p.set_primary_vtable(0x00AA_8314);
            p.set_secondary_vtable(0x00AA_82FC);
            clear_tree(this.add(0x1C), true);
            let head = p.second_tree();
            if !head.is_null() { free(head); }
            clear_tree(this.add(0x10), false);
            let head = p.first_tree();
            if !head.is_null() { free(head); }
            let service = p.service();
            if !service.is_null() { vcall!(service, 8 => fn() -> u32); }
            destroy_interface(this.add(4))
        }
    }
}
port! {
    /// Deleting-destructor entry from the secondary interface.
    #[port(address = 0x0062_0810, owner = "networktool", name = "DeletePreviewFromSecondaryInterface", naked)]
    pub unsafe extern "C" fn fc4p_00620810() {
        core::arch::naked_asm!("sub ecx, 4", "jmp {body}", body = sym deleting_from_secondary)
    }
}
unsafe extern "thiscall" fn deleting_from_secondary(this: *mut u8, flags: u32) -> u64 {
    unsafe { delete_preview(this, flags) }
}
register_port!(0x0062_08D0, fc4p_006208d0, "ClearPreviewRules", clear_rules, "ret");
unsafe extern "fastcall" fn clear_rules(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let p = Preview::from_ptr(this);
        let first = p.first_count();
        let mut edx = frame.read();
        if first != 0 {
            edx = clear_tree(this.add(0x10), false);
        }
        let second = p.second_count();
        let eax = if second != 0 {
            edx = clear_tree(this.add(0x1C), true);
            p.second_tree() as u32
        } else {
            second
        };
        registers(eax, edx)
    }
}
port! {
    /// Construct both empty tree headers and the preview's byte flags. Leave structure padding untouched.
    #[port(address = 0x0062_0AD0, owner = "networktool", name = "ConstructNetworkPreview")]
    pub unsafe extern "thiscall" fn fc4p_00620ad0(this: *mut u8) -> *mut u8 {
        unsafe {
            let p = Preview::from_ptr(this);
            p.set_primary_vtable(0x00A8_81C0);
            construct_interface(this.add(4));
            p.set_primary_vtable(0x00AA_8314);
            p.set_secondary_vtable(0x00AA_82FC);
            p.set_service(core::ptr::null_mut());
            for (offset, size) in [(0x10, 0x7Cu32), (0x1C, 0x80)] {
                write(this, offset, core::ptr::null_mut::<u8>());
                let head = allocate(size);
                write(this, offset, head);
                write(this, offset + 4, 0u32);
                write(head, 0, 0u8);
                let head: *mut u8 = read(this, offset);
                write(head, 4, 0u32);
                let head: *mut u8 = read(this, offset);
                write(head, 8, head);
                let head: *mut u8 = read(this, offset);
                write(head, 0xC, head);
            }
            for offset in (0x28..0x48).step_by(4) { write(this, offset, 0u32); }
            for (offset, value) in [(0x48, 1u8), (0x49, 0), (0x4B, 1), (0x4C, 1),
                (0x4D, 0), (0x4F, 1), (0x50, 0), (0x51, 0)] { write(this, offset, value); }
            write(this, 0x52, 0u16);
            write(this, 0x4A, 3u8);
            write(this, 0x4E, 3u8);
            p.set_initialized(0);
        }
        this
    }
}
port! {
    /// Destroy the preview and free its storage only when bit zero of flags is set.
    #[port(address = 0x0062_0BA0, owner = "networktool", name = "DeleteNetworkPreview")]
    pub unsafe extern "thiscall" fn fc4p_00620ba0(this: *mut u8, flags: u32) -> u64 {
        unsafe {
            let mut result = destroy_preview(this);
            if flags & 1 != 0 { result = delete_storage(this); }
            registers(this as u32, (result >> 32) as u32)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0061fbc0::PORT,
    fc4p_00620770::PORT,
    fc4p_00620810::PORT,
    fc4p_006208d0::PORT,
    fc4p_00620ad0::PORT,
    fc4p_00620ba0::PORT,
];

#[cfg(test)]
mod tests;
