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

game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }
game_fn! { fn unregister_iterator(this: *mut u8, iterator: *mut *mut u8) -> u64 = "thiscall" 0x0080_9580; }
game_fn! { fn free_iterator(iterator: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { fn inflate_rectangle(this: *mut u32, amount: i32) -> u64 = "thiscall" 0x006C_6010; }

layout! {
    /// A safe lot iterator. Its owner can be cleared by callbacks while iteration is in progress.
    pub struct LotIterator {
        0x00 => current / set_current: *mut u8,
        0x04 => owner / set_owner: *mut u8,
    }
}

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

port! {
    /// Marshals both entry result words for the unchanged-mode path and keeps the original four-byte pop.
    #[port(address = 0x006C_77D0, owner = "lotdeveloper", name = "cSC4LotDeveloper::SetForceEmptyLotBaseTextures", naked)]
    pub unsafe extern "C" fn fc4p_006c77d0() {
        core::arch::naked_asm!(
            "push dword ptr [esp + 4]", "push edx", "mov edx, eax", "call {}", "ret 4",
            sym set_force_empty_textures,
        );
    }
}

unsafe extern "fastcall" fn set_force_empty_textures(this: *mut u8, entry_eax: u32, entry_edx: u32, mode: u32) -> u64 {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let mode = mode as u8;
        if mode == developer.force_empty() {
            return (u64::from(entry_edx) << 32) | u64::from((entry_eax & !255) | u32::from(mode));
        }
        developer.set_force_empty(mode);
        let manager = lot_manager().read();
        let mut call = virtual_call(manager, 0x3C, &[], table(manager), entry_edx, false);
        let iterator_pointer = call.eax as *mut u8;
        let iterator = LotIterator::from_ptr(iterator_pointer);
        loop {
            let owner = iterator.owner();
            if owner.is_null() {
                break;
            }
            let current = iterator.current();
            if current == owner.cast::<*mut u8>().read_unaligned() {
                break;
            }
            iterator.set_current(OccupantNode::from_ptr(current).next());
            let lot = OccupantNode::from_ptr(current).occupant();
            call.eax = (current as u32 & !255) | u32::from(developer.force_empty());
            if call.eax as u8 != 0 {
                call = virtual_call(this, 0xAC, &[lot as u32], call.eax, table(this), false);
            } else {
                // The table pointer is captured before the lot's configuration callback; its method
                // word is read afterwards, so edits to the old table are still visible.
                let saved_table = table(this);
                call = virtual_call(lot, 0x24, &[1], table(lot), call.edx, false);
                let target = (saved_table as *const usize).add(0xB0 / 4).read_unaligned();
                let args = [lot as u32, call.eax];
                let mut normal = RawCall::new(Conv::Thiscall, target, this as u32, &args);
                normal.eax = call.eax;
                normal.edx = call.edx;
                call_raw(&mut normal);
                call = normal;
            }
        }
        let owner = iterator.owner();
        if !owner.is_null() {
            let mut iterator_word = iterator_pointer;
            unregister_iterator(owner.add(4), &mut iterator_word);
        }
        free_iterator(iterator_pointer)
    }
}

port! {
    /// Copies a rectangle, inflates the copy through the game's helper, and writes four output words.
    /// EAX and ECX both return the output pointer; EDX returns its fourth word.
    #[port(address = 0x006C_6860, owner = "lotdeveloper", name = "InflatedLotRectangle", naked)]
    pub unsafe extern "C" fn fc4p_006c6860() {
        core::arch::naked_asm!(
            "push dword ptr [esp + 8]", "push dword ptr [esp + 8]", "call {}", "mov ecx, eax", "ret 8",
            sym inflated_rectangle,
        );
    }
}

unsafe extern "thiscall" fn inflated_rectangle(this: *mut u8, output: *mut u32, amount: i32) -> u64 {
    unsafe {
        let mut copy = [0u32; 4];
        for (i, value) in copy.iter_mut().enumerate() {
            *value = this.add(i * 4).cast::<u32>().read_unaligned();
        }
        inflate_rectangle(copy.as_mut_ptr(), amount);
        for (i, value) in copy.iter().enumerate() {
            output.add(i).write_unaligned(*value);
        }
        (u64::from(copy[3]) << 32) | u64::from(output as u32)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c62f0::PORT,
    fc4p_006c6340::PORT,
    fc4p_006c6860::PORT,
    fc4p_006c6ca0::PORT,
    fc4p_006c6cf0::PORT,
    fc4p_006c6e90::PORT,
    fc4p_006c7320::PORT,
    fc4p_006c7360::PORT,
    fc4p_006c7370::PORT,
    fc4p_006c77d0::PORT,
];

#[cfg(test)]
mod tests;
