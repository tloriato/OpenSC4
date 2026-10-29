// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Resource construction, copying, destruction and pooled cloning.

use super::*;

game_fn! { fn resource_construct(this: *mut u8) = "thiscall" 0x0097_5633; }
game_fn! { fn map_initialize(this: *mut u8, count: u32) = "thiscall" 0x0082_9BC0; }
game_fn! { fn map_copy(this: *mut u8, source: *mut u8) = "thiscall" 0x0054_0160; }
game_fn! { #[allow(dead_code)] fn map_clear(this: *mut u8) = "thiscall" 0x0058_4660; }
game_fn! { #[allow(dead_code)] fn free(pointer: *mut u8) = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn resource_destroy(this: *mut u8) = "thiscall" 0x0097_538E; }
game_fn! { #[allow(dead_code)] fn pool_release_page(this: *mut u8, page: *mut u8) = "thiscall" 0x0090_D031; }
game_fn! { #[allow(dead_code)] fn pool_queue_page(this: *mut u8, page: *mut u8) = "thiscall" 0x0090_CFF9; }
game_fn! { fn pool_allocate(this: *mut u8) -> *mut u8 = "thiscall" 0x0090_CF9A; }
game_fn! { fn construct_copy(this: *mut u8, source: *mut u8) -> u64 = "thiscall" 0x0054_08B0; }
game_fn! {
    #[allow(dead_code)]
    fn query_interface(this: *mut u8, iid: u32, out: *mut *mut u8) -> u64 = "thiscall" 0x0053_CFA0;
}
game_fn! {
    #[allow(dead_code)]
    fn destroy(this: *mut u8, flags: u32) -> u32 = "thiscall" 0x0054_0820;
}
game_data! { static pool: u8 = 0x00B0_9FEC; }
game_data! { static current_page: *mut u8 = 0x00B0_9FF0; }

port! {
    /// Adjusts the resource interface by four bytes and forwards QueryInterface to its original entry.
    #[port(address = 0x0054_03D0, owner = "pathinfo", name = "cSC4PathInfo::ResourceQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_005403d0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0053cfa0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym super::fc4p_0053cfa0);
    }
}

port! {
    /// Adjusts the resource interface by four bytes and forwards the deleting destructor.
    #[port(address = 0x0054_03E0, owner = "pathinfo", name = "cSC4PathInfo::ResourceDeletingDestructor", naked)]
    pub unsafe extern "C" fn fc4p_005403e0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00540820");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym fc4p_00540820);
    }
}

// These stores deliberately leave the first word of the map header alone.
unsafe fn empty_map(info: PathInfo) {
    info.set_buckets(core::ptr::null_mut());
    info.set_buckets_end(core::ptr::null_mut());
    info.set_buckets_capacity(core::ptr::null_mut());
    info.set_path_count(0);
}

// Some lifetime callees preserve EDX, so it must pass through each call rather than a Rust temporary register.
unsafe fn lifetime_call(address: u32, conv: Conv, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> (u32, u32) {
    let mut call = RawCall::new(conv, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe { call_raw(&mut call) };
    (call.eax, call.edx)
}

port! {
    /// Constructs the resource base, initializes 100 map buckets, and clears the shared and per-type flags.
    /// Bytes +0x39 onward and the map header's first word are left to their existing owners.
    #[port(address = 0x0054_07D0, owner = "pathinfo", name = "cSC4PathInfo::cSC4PathInfo")]
    pub unsafe extern "thiscall" fn fc4p_005407d0(this: *mut u8) -> *mut u8 {
        unsafe {
            let info = PathInfo::from_ptr(this);
            info.set_vtable(0x00A9_9D78);
            resource_construct(this.add(4));
            info.set_vtable(0x00A9_9E70);
            info.set_resource_vtable(0x00A9_9E38);
            empty_map(info);
            map_initialize(this.add(0x1C), 100);
            info.set_flags(0);
            write_unaligned(this.add(0x31).cast::<u32>(), 0);
            write_unaligned(this.add(0x35).cast::<u32>(), 0);
            this
        }
    }
}

port! {
    /// Clears the map, frees its bucket allocation, and destroys the resource base. Flag bit zero then
    /// returns this object to its page's free list, releasing an empty page or queuing a newly available one.
    #[port(address = 0x0054_0820, owner = "pathinfo", name = "cSC4PathInfo::DeletingDestructor")]
    pub unsafe extern "fastcall" fn fc4p_00540820(this: *mut u8, entry_edx: u32, flags: u32) -> u64 {
        unsafe {
            let info = PathInfo::from_ptr(this);
            info.set_vtable(0x00A9_9E70);
            info.set_resource_vtable(0x00A9_9E38);
            let (mut eax, mut edx) = lifetime_call(0x0058_4660, Conv::Thiscall, this.add(0x1C), &[], 0, entry_edx);
            let buckets = info.buckets();
            if !buckets.is_null() {
                (eax, edx) = lifetime_call(0x0090_CF63, Conv::Cdecl, core::ptr::null_mut(), &[buckets as u32], eax, edx);
            }
            (_, edx) = lifetime_call(0x0097_538E, Conv::Thiscall, this.add(4), &[], eax, edx);
            if flags & 1 != 0 {
                let page = (this as u32 & 0xFFFF_F000) as *mut u8;
                write_unaligned(this.cast::<u32>(), read_unaligned(page.add(8).cast()));
                let count = read_unaligned(page.add(12).cast::<u32>()).wrapping_sub(1);
                write_unaligned(page.add(8).cast::<*mut u8>(), this);
                write_unaligned(page.add(12).cast::<u32>(), count);
                if count == 0 {
                    (_, edx) = lifetime_call(0x0090_D031, Conv::Thiscall, pool(), &[page as u32], page as u32, edx);
                } else if count == 0x4000_0033 {
                    (_, edx) = lifetime_call(0x0090_CFF9, Conv::Thiscall, pool(), &[page as u32], page as u32, edx);
                }
            }
            (u64::from(edx) << 32) | u64::from(this as u32)
        }
    }
}

port! {
    /// Constructs a fresh resource base, copies the map through its own copy routine, and copies nine flag bytes.
    #[port(address = 0x0054_08B0, owner = "pathinfo", name = "cSC4PathInfo::cSC4PathInfo (copy)")]
    pub unsafe extern "thiscall" fn fc4p_005408b0(this: *mut u8, source: *mut u8) -> *mut u8 {
        unsafe {
            let info = PathInfo::from_ptr(this);
            info.set_vtable(0x00A9_9D78);
            resource_construct(this.add(4));
            info.set_resource_vtable(0x00A9_9E38);
            info.set_vtable(0x00A9_9E70);
            empty_map(info);
            map_copy(this.add(0x1C), source.add(0x1C));
            info.set_flags(PathInfo::from_ptr(source).flags());
            for offset in 0x31..0x39 {
                write_unaligned(this.add(offset), read_unaligned(source.add(offset)));
            }
            this
        }
    }
}

port! {
    /// Takes an object from the current pool page, growing the pool if needed, copy-constructs it and AddRefs it.
    /// The page pointer is reloaded for each free-list and reference-count update.
    #[port(address = 0x0054_0910, owner = "pathinfo", name = "cSC4PathInfo::Clone")]
    pub unsafe extern "thiscall" fn fc4p_00540910(this: *mut u8) -> u64 {
        unsafe {
            let page = read_unaligned(current_page());
            let mut storage = read_unaligned(page.add(8).cast::<*mut u8>());
            if storage.is_null() { storage = pool_allocate(pool()); }
            let next = read_unaligned(storage.cast::<*mut u8>());
            write_unaligned(read_unaligned(current_page()).add(8).cast::<*mut u8>(), next);
            let page = read_unaligned(current_page());
            let count = read_unaligned(page.add(12).cast::<u32>()).wrapping_add(1);
            write_unaligned(page.add(12).cast::<u32>(), count);
            let copied = construct_copy(storage, this);
            let object = copied as u32 as *mut u8;
            let mut call = RawCall::new(Conv::Thiscall, vslot(object, 4), object as u32, &[]);
            call.eax = read_unaligned(object.cast::<u32>());
            call.edx = (copied >> 32) as u32;
            call_raw(&mut call);
            (u64::from(call.edx) << 32) | u64::from(object as u32)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_005403d0::PORT,
    fc4p_005403e0::PORT,
    fc4p_005407d0::PORT,
    fc4p_00540820::PORT,
    fc4p_005408b0::PORT,
    fc4p_00540910::PORT,
];

#[cfg(test)]
mod tests;
