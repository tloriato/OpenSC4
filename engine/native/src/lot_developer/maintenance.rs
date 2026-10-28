// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Population transfers, boundary lookup and shared occupant containers.

use super::*;

layout! {
    /// A circular list node, including the sentinel's two links.
    pub struct OccupantNode {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => occupant / set_occupant: *mut u8,
    }
}

game_fn! { fn allocate_node(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn free_storage(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn find_boundary(this: *mut u8, output: *mut *mut u8, key: *const i32) -> u64 = "thiscall" 0x006C_0030; }
game_fn! { #[allow(dead_code)] fn release_developer(this: *mut u8, edx: u32) -> u64 = "fastcall" 0x006C_49D0; }

port! {
    /// Removes inserted occupants, shuts each one down, and releases it. The sentinel is captured once;
    /// each next link and the developer's occupant manager are read after the preceding callbacks.
    #[port(address = 0x006C_62F0, owner = "lotdeveloper", name = "RemoveOccupantList")]
    pub unsafe extern "thiscall" fn fc4p_006c62f0(this: *mut u8, list: *mut *mut u8) -> u32 {
        unsafe {
            let developer = LotDeveloper::from_ptr(this);
            let sentinel = list.read_unaligned();
            let mut current = OccupantNode::from_ptr(sentinel).next();
            let mut eax = list as u32;
            while current != sentinel {
                let node = OccupantNode::from_ptr(current);
                let occupant = node.occupant();
                if vcall!(occupant, 0x80 => fn(u32) -> u32, 1) as u8 != 0 {
                    vcall!(developer.occupant_manager(), 0x54 => fn(*mut u8, u32, u32) -> u32, occupant, 0, 0);
                }
                vcall!(occupant, 0x10 => fn() -> u32);
                eax = vcall!(occupant, 8 => fn() -> u32);
                current = node.next();
            }
            eax
        }
    }
}

port! {
    /// Resets every member before accumulating any population. A missing designate receives its own
    /// population. The list's sentinel word is reread at every loop boundary and between the two passes.
    #[port(address = 0x006C_6340, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateTravelPopulations")]
    pub unsafe extern "fastcall" fn fc4p_006c6340(_this: *mut u8, entry_edx: u32, unit: *mut u8) -> u64 {
        unsafe {
            let mut call = virtual_call(unit, 0x14, &[], table(unit), entry_edx, false);
            let list = call.eax as *mut *mut u8;
            let mut sentinel = list.read_unaligned();
            let mut current = OccupantNode::from_ptr(sentinel).next();
            call.eax = sentinel as u32;
            while current != sentinel {
                let node = OccupantNode::from_ptr(current);
                let member = node.occupant();
                call = virtual_call(member, 0xE4, &[], call.eax, table(member), false);
                current = node.next();
                sentinel = list.read_unaligned();
            }
            sentinel = list.read_unaligned();
            current = OccupantNode::from_ptr(sentinel).next();
            call.eax = sentinel as u32;
            while current != sentinel {
                let node = OccupantNode::from_ptr(current);
                let member = node.occupant();
                call = virtual_call(member, 0xEC, &[], table(member), call.edx, false);
                let receiver = call.eax as *mut u8;
                if receiver.is_null() {
                    call = virtual_call(member, 0xE8, &[member as u32], table(member), call.edx, false);
                } else {
                    call = virtual_call(receiver, 0xE8, &[member as u32], call.eax, table(receiver), false);
                }
                current = node.next();
                sentinel = list.read_unaligned();
            }
            result(call)
        }
    }
}

port! {
    /// Copies a list range before a fixed destination node, AddRefing non-null occupants before linking
    /// each new node. The allocator supplies twelve bytes; payload padding is never initialized.
    #[port(address = 0x006C_6CA0, owner = "lotdeveloper", name = "CopyOccupantListRange")]
    pub unsafe extern "stdcall" fn fc4p_006c6ca0(destination: *mut u8, begin: *mut u8, end: *mut u8, _allocator: *mut u8) {
        unsafe {
            let mut current = begin;
            while current != end {
                let allocation = allocate_node(12);
                let payload = allocation.wrapping_add(8).cast::<*mut u8>();
                if !payload.is_null() {
                    let occupant = OccupantNode::from_ptr(current).occupant();
                    payload.write_unaligned(occupant);
                    if !occupant.is_null() { vcall!(occupant, 4 => fn() -> u32); }
                }
                let destination = OccupantNode::from_ptr(destination);
                let previous = destination.previous();
                let added = OccupantNode::from_ptr(allocation);
                added.set_next(destination.as_ptr());
                added.set_previous(previous);
                OccupantNode::from_ptr(previous).set_next(allocation);
                destination.set_previous(allocation);
                current = OccupantNode::from_ptr(current).next();
            }
        }
    }
}

port! {
    /// Packs four signed side codes with wrapping shifts and additions, looks up that key, then key zero
    /// on a miss. Even a second miss reads the sentinel's texture, rotation and flip bytes.
    #[port(address = 0x006C_6CF0, owner = "lotdeveloper", name = "cSC4LotDeveloper::GetBoundaryLotTexture")]
    pub unsafe extern "thiscall" fn fc4p_006c6cf0(this: *mut u8, sides: *const i32, texture: *mut u32, rotation: *mut u8, flip: *mut u8) -> u64 {
        unsafe {
            let mut key = sides.read_unaligned();
            for i in 1..4 { key = key.wrapping_shl(4).wrapping_add(sides.add(i).read_unaligned()); }
            let map = this.add(0xFC);
            let mut node = core::ptr::null_mut();
            find_boundary(map, &mut node, &key);
            if node == map.cast::<*mut u8>().read_unaligned() {
                key = 0;
                find_boundary(map, &mut node, &key);
            }
            texture.write_unaligned(node.add(0x14).cast::<u32>().read_unaligned());
            rotation.write(node.add(0x18).read());
            let value = node.add(0x19).read();
            flip.write(value);
            (u64::from(rotation as u32) << 32) | u64::from((node as u32 & !255) | u32::from(value))
        }
    }
}

port! {
    /// Keeps entry EAX and EDX available to the Rust body when the vector has no non-null elements or storage.
    #[port(address = 0x006C_6E90, owner = "lotdeveloper", name = "DestroyOccupantPointerVector", naked)]
    pub unsafe extern "C" fn fc4p_006c6e90() {
        core::arch::naked_asm!("push edx", "mov edx, eax", "call {}", "ret", sym destroy_pointer_vector);
    }
}

unsafe extern "fastcall" fn destroy_pointer_vector(this: *mut u8, entry_eax: u32, entry_edx: u32) -> u64 {
    unsafe {
        let mut current = this.cast::<*mut u8>().read_unaligned();
        let end = this.add(4).cast::<*mut u8>().read_unaligned();
        let mut returned = (u64::from(entry_edx) << 32) | u64::from(entry_eax);
        while current != end {
            let occupant = current.cast::<*mut u8>().read_unaligned();
            if !occupant.is_null() {
                returned = result(virtual_call(
                    occupant,
                    8,
                    &[],
                    table(occupant),
                    (returned >> 32) as u32,
                    false,
                ));
            }
            current = current.wrapping_add(4);
        }
        let storage = this.cast::<*mut u8>().read_unaligned();
        if !storage.is_null() {
            returned = free_storage(storage);
        }
        returned
    }
}

port! {
    /// Removes the lot from the unit returned by the lot, then marks business-unit populations dirty.
    #[port(address = 0x006C_7320, owner = "lotdeveloper", name = "cSC4LotDeveloper::RemoveLotFromItsBusinessUnit")]
    pub unsafe extern "fastcall" fn fc4p_006c7320(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let call = virtual_call(lot, 0x64, &[], table(lot), entry_edx, false);
            let unit = call.eax as *mut u8;
            let call = virtual_call(unit, 0x10, &[lot as u32], call.eax, table(unit), false);
            LotDeveloper::from_ptr(this).set_business_units_dirty(1);
            result(call)
        }
    }
}

#[cfg(test)]
unsafe extern "fastcall" fn release_for_test(this: *mut u8, edx: u32) -> u64 {
    unsafe { release_developer(this, edx) }
}

port! {
    /// Adjusts the message interface pointer and forwards the original Release call.
    #[port(address = 0x006C_7360, owner = "lotdeveloper", name = "cSC4LotDeveloper::MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_006c7360() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x006c49d0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym release_for_test);
    }
}

port! {
    /// Adjusts the serializable interface pointer and forwards the original Release call.
    #[port(address = 0x006C_7370, owner = "lotdeveloper", name = "cSC4LotDeveloper::SerializableRelease", naked)]
    pub unsafe extern "C" fn fc4p_006c7370() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006c49d0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym release_for_test);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c62f0::PORT,
    fc4p_006c6340::PORT,
    fc4p_006c6ca0::PORT,
    fc4p_006c6cf0::PORT,
    fc4p_006c6e90::PORT,
    fc4p_006c7320::PORT,
    fc4p_006c7360::PORT,
    fc4p_006c7370::PORT,
];

#[cfg(test)]
mod tests;
