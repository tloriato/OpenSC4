// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Shared helpers, group 06: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

layout! {
    /// The two services used to check the first network cell.
    struct NetworkCheck {
        0x44 => terrain / set_terrain: *mut u8,
        0x60 => coordinates / set_coordinates: *mut u8,
    }
}

layout! {
    /// A byte range, immediately after its owner's vtable word.
    struct ByteRange {
        0x00 => first / set_first: *mut u8,
        0x04 => last / set_last: *mut u8,
    }
}

layout! {
    /// A tree node ordered by two signed coordinates.
    struct CoordinateNode {
        0x08 => left / set_left: *mut u8,
        0x0C => right / set_right: *mut u8,
        0x10 => x / set_x: i32,
        0x14 => z / set_z: i32,
    }
}

// These accesses use the enclosing entry's live game objects, including unaligned fields.
unsafe fn word(p: *const u8, offset: usize) -> u32 {
    unsafe { read_unaligned(p.wrapping_add(offset).cast()) }
}

unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { write_unaligned(p.wrapping_add(offset).cast(), value) }
}

unsafe fn pointer(p: *const u8, offset: usize) -> *mut u8 {
    unsafe { word(p, offset) as *mut u8 }
}

game_fn! { fn network_is_active(object: *mut u8) -> u64 = "C" 0x0063_8600; }
game_fn! { fn base_destroy(this: *mut u8) -> u64 = "thiscall" 0x005E_95C0; }
game_fn! { fn free_object(object: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { fn underground_destroy(this: *mut u8) -> u64 = "thiscall" 0x0064_8E90; }
game_fn! { fn lookup_network_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0063_3220; }
game_fn! { #[allow(dead_code)] fn property_type(this: *mut u8) -> u32 = "thiscall" 0x0063_DDF0; }

fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

port! {
    /// Copies sixteen-byte records forward, skipping construction at a null destination.
    /// Returns the destination end in EAX and the source end in EDX.
    #[port(address = 0x0063_7E20, owner = "shared", name = "CopySixteenByteRecords")]
    pub unsafe extern "C" fn fc4p_00637e20(first: *const u8, last: *const u8, out: *mut u8) -> u64 {
        let (mut first, mut out) = (first, out);
        unsafe {
            while first != last {
                if !out.is_null() {
                    for offset in [0, 4, 8, 12] {
                        put(out, offset, word(first, offset));
                    }
                }
                first = first.wrapping_add(16);
                out = out.wrapping_add(16);
            }
            (u64::from(last as u32) << 32) | u64::from(out as u32)
        }
    }
}

port! {
    /// Fills twelve-byte records, reading each word of the source again for every record.
    #[port(address = 0x0063_7E60, owner = "shared", name = "FillTwelveByteRecords")]
    pub unsafe extern "C" fn fc4p_00637e60(first: *mut u8, last: *mut u8, value: *const u8) -> *mut u8 {
        let mut first = first;
        unsafe {
            while first != last {
                for offset in [0, 4, 8] {
                    put(first, offset, word(value, offset));
                }
                first = first.wrapping_add(12);
            }
            first
        }
    }
}

port! {
    /// Delegates to the retained object, then checks the supplied network interface on success.
    #[port(address = 0x0063_85D0, owner = "shared", name = "DelegateAndCheckNetworkActive")]
    pub unsafe extern "thiscall" fn fc4p_006385d0(this: *mut u8, object: *mut u8) -> u64 {
        unsafe {
            let delegated = vcall!(pointer(this, 0x0C), 0x0C => fn(*mut u8) -> u64, object);
            if delegated as u8 == 0 {
                return delegated & !0xFFFF_FFFF;
            }
            let checked = network_is_active(object);
            (checked & !0xFFFF_FFFF) | u64::from(checked as u8 != 0)
        }
    }
}

port! {
    /// Queries the network interface, tests its active state, then releases any returned interface.
    /// Only AL is replaced in the final release result.
    #[port(address = 0x0063_8600, owner = "shared", name = "CheckNetworkActive", naked)]
    pub unsafe extern "C" fn fc4p_00638600() {
        core::arch::naked_asm!("mov ecx, edx", "mov edx, esp", "jmp {body}", body = sym check_network_active);
    }
}

unsafe extern "fastcall" fn check_network_active(entry_edx: u32, stack: *const u32) -> u64 {
    unsafe {
        let object = stack.add(1).read_unaligned() as *mut u8;
        if object.is_null() {
            return registers(0, entry_edx);
        }
        let mut network = core::ptr::null_mut();
        let mut result = vcall!(object, 0 => fn(u32, *mut *mut u8) -> u64, 0xA821_EF94, &mut network);
        let active = if result as u8 != 0 {
            result = vcall!(network, 0x54 => fn() -> u64);
            result as u8 != 0
        } else {
            false
        };
        if !network.is_null() {
            result = vcall!(network, 8 => fn() -> u64);
        } else {
            result &= !0xFFFF_FFFF;
        }
        (result & !0xFF) | u64::from(active)
    }
}

port! {
    /// Releases the retained delegate, destroys the base, and optionally frees the object.
    #[port(address = 0x0063_8650, owner = "shared", name = "DeleteRetainedDelegate")]
    pub unsafe extern "thiscall" fn fc4p_00638650(this: *mut u8, flags: u32) -> u64 {
        unsafe {
            let delegate = pointer(this, 0x0C);
            if !delegate.is_null() { vcall!(delegate, 8 => fn() -> u32); }
            let mut result = base_destroy(this);
            if flags & 1 != 0 { result = free_object(this); }
            (result & !0xFFFF_FFFF) | u64::from(this as u32)
        }
    }
}

port! {
    /// Rejects an existing inactive network cell, clearing the output only on rejection.
    #[port(address = 0x0063_8710, owner = "shared", name = "CheckFirstNetworkCell")]
    pub unsafe extern "thiscall" fn fc4p_00638710(this: *mut u8, output: *mut u32) -> u64 {
        unsafe {
            let mut result = vcall!(this, 0xFC => fn() -> u64);
            if result as u8 != 0 {
                let check = NetworkCheck::from_ptr(this);
                let coordinates = check.coordinates();
                let x = word(coordinates, 0);
                let z = word(coordinates, 4);
                result = vcall!(check.terrain(), 0x88 => fn(u32, u32) -> u64, x, z);
                if result as u8 != 0 {
                    result = lookup_network_cell(this, z.wrapping_shl(16).wrapping_add(x));
                    let cell = result as u32 as *mut u8;
                    if cell.is_null() || *cell.add(0x56) == 0 {
                        output.write_unaligned(0);
                        return (result & !0xFFFF_FFFF) | u64::from((output as u32) & !0xFF);
                    }
                }
            }
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Compares the byte ranges stored after each object's vtable word.
    #[port(address = 0x0063_C020, owner = "shared", name = "EqualByteRanges")]
    pub unsafe extern "stdcall" fn fc4p_0063c020(a: *mut u8, b: *mut u8) -> u64 {
        unsafe {
            let a = if a.is_null() { a } else { a.wrapping_add(4) };
            let b = if b.is_null() { b } else { b.wrapping_add(4) };
            let ar = ByteRange::from_ptr(a);
            let br = ByteRange::from_ptr(b);
            let x = ar.first();
            let y = br.first();
            let len = (ar.last() as u32).wrapping_sub(x as u32);
            if len != (br.last() as u32).wrapping_sub(y as u32) { return registers(0, b as u32); }
            for i in 0..len {
                if *x.wrapping_add(i as usize) != *y.wrapping_add(i as usize) { return 0; }
            }
            1
        }
    }
}

port! {
    /// Finds the first tree node whose signed coordinate pair is at least the requested pair.
    #[port(address = 0x0063_C070, owner = "shared", name = "LowerBoundCoordinatePair")]
    pub unsafe extern "thiscall" fn fc4p_0063c070(this: *mut u8, key: *const i32) -> *mut u8 {
        unsafe {
            let mut result = pointer(this, 0);
            let mut node = pointer(result, 4);
            if !node.is_null() {
                let x = key.read_unaligned();
                loop {
                    let view = CoordinateNode::from_ptr(node);
                    let nx = view.x();
                    if x < nx || (x == nx && view.z() >= key.add(1).read_unaligned()) {
                        result = node;
                        node = view.left();
                    } else {
                        node = view.right();
                    }
                    if node.is_null() { break; }
                }
            }
            result
        }
    }
}

port! {
    /// Copies the property payload only for type 0x100; AL reports whether it was copied.
    #[port(address = 0x0063_E9D0, owner = "shared", name = "ReadTypedPropertyWord", naked)]
    pub unsafe extern "C" fn fc4p_0063e9d0() {
        core::arch::naked_asm!("push edx", "push ecx", "push eax", "mov edx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "ret 4", body = sym read_typed_property_word);
    }
}

unsafe extern "fastcall" fn read_typed_property_word(this: *mut u8, frame: *mut u32) {
    unsafe {
        let target = crate::abi::target(0x0063_DDF0);
        let (mut eax, mut ecx, mut edx) = (frame.read(), this as u32, frame.add(2).read());
        core::arch::asm!("call {target}", target = in(reg) target,
            inlateout("eax") eax, inlateout("ecx") ecx, inlateout("edx") edx, clobber_abi("C"));
        if eax == 0x100 {
            eax = word(this, 0x20);
            ecx = frame.add(4).read();
            (ecx as *mut u32).write_unaligned(eax);
            eax = (eax & !0xFF) | 1;
        } else {
            eax &= !0xFF;
        }
        frame.write(eax);
        frame.add(1).write(ecx);
        frame.add(2).write(edx);
    }
}

// Forwarding entries preserve the caller's registers and pass all stack words through unchanged.
#[cfg(test)]
extern "C" fn forwarding_target(address: u32) -> usize {
    crate::abi::target(address)
}

macro_rules! forward_interface {
    ($export:ident, $address:literal, $adjust:literal, $virtual_adjust:literal, $target:literal, $name:literal) => {
        port! {
            /// Adjusts the interface pointer and jumps to the original method entry.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!(
                    $virtual_adjust, $adjust, "jmp dword ptr [2f]", "2: .long {target}",
                    target = const $target,
                );
                #[cfg(test)]
                core::arch::naked_asm!(
                    $virtual_adjust, $adjust, "push eax", "push ecx", "push edx",
                    "push {target}", "call {resolve}", "add esp, 4",
                    "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
                    target = const $target, resolve = sym forwarding_target,
                );
            }
        }
    };
}

// The declarations name the original call targets; forwarding entries keep their complete incoming frame.
#[allow(dead_code)]
mod forwarding {
    game_fn! { pub fn underground_add_ref(this: *mut u8) -> u32 = "thiscall" 0x0065_4120; }
    game_fn! { pub fn underground_release(this: *mut u8) -> u32 = "thiscall" 0x0065_4130; }
    game_fn! { pub fn set_model(this: *mut u8, model: *mut u8) -> u32 = "thiscall" 0x0080_1860; }
    game_fn! { pub fn get_model_state(this: *mut u8) -> u32 = "thiscall" 0x0080_1890; }
    game_fn! { pub fn set_model_state(this: *mut u8, state: u32) -> u32 = "thiscall" 0x0080_18A0; }
    game_fn! { pub fn initialize_connection(this: *mut u8, context: *mut u8) -> u32 = "thiscall" 0x0065_4140; }
    game_fn! { pub fn read_connection(this: *mut u8, stream: *mut u8) -> u32 = "thiscall" 0x0065_4500; }
    game_fn! { pub fn write_connection(this: *mut u8, stream: *mut u8) -> u32 = "thiscall" 0x0065_4590; }
    game_fn! { pub fn get_model_flags(this: *mut u8) -> u32 = "thiscall" 0x0080_18E0; }
    game_fn! { pub fn copy_model_data(this: *mut u8, output: *mut u8) -> u32 = "thiscall" 0x0080_1E10; }
    game_fn! { pub fn bridge_add_ref(this: *mut u8) -> u32 = "thiscall" 0x0065_A860; }
    game_fn! { pub fn bridge_release(this: *mut u8) -> u32 = "thiscall" 0x0065_A870; }
    game_fn! { pub fn occupant_release(this: *mut u8) -> u32 = "thiscall" 0x005E_8DF0; }
    game_fn! { pub fn prop_add_ref(this: *mut u8) -> u32 = "thiscall" 0x0064_91C0; }
    game_fn! { pub fn construction_release(this: *mut u8) -> u32 = "thiscall" 0x0064_91D0; }
    game_fn! { pub fn pipe_shutdown(this: *mut u8) -> u32 = "thiscall" 0x0065_2CF0; }
    game_fn! { pub fn count_ports(this: *mut u8, kind: u32, filter: u32) -> u32 = "thiscall" 0x006E_94C0; }
    game_fn! { pub fn get_port_lots(this: *mut u8, output: *mut u8) -> u32 = "thiscall" 0x006E_A2F0; }
    game_fn! { pub fn add_ref_at_eight(this: *mut u8) -> u32 = "thiscall" 0x004F_7BA0; }
    game_fn! { pub fn port_release(this: *mut u8) -> u32 = "thiscall" 0x006E_8F90; }
    game_fn! { pub fn landfill_release(this: *mut u8) -> u32 = "thiscall" 0x006D_5140; }
}

forward_interface!(
    fc4p_006480d0,
    0x0064_80D0,
    "sub ecx, 4",
    "",
    0x0065_4120,
    "AddRefConnectionFromSecondInterface"
);
forward_interface!(
    fc4p_006480e0,
    0x0064_80E0,
    "sub ecx, 4",
    "",
    0x0065_4130,
    "ReleaseConnectionFromSecondInterface"
);
forward_interface!(
    fc4p_006481d0,
    0x0064_81D0,
    "sub ecx, 588",
    "sub ecx, [ecx - 4]",
    0x0065_4120,
    "AddRefConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_006481e0,
    0x0064_81E0,
    "sub ecx, 588",
    "sub ecx, [ecx - 4]",
    0x0065_4130,
    "ReleaseConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648200,
    0x0064_8200,
    "sub ecx, 484",
    "sub ecx, [ecx - 4]",
    0x0080_1860,
    "SetModelFromVirtualBase"
);
forward_interface!(
    fc4p_00648210,
    0x0064_8210,
    "sub ecx, 484",
    "sub ecx, [ecx - 4]",
    0x0080_1890,
    "GetModelStateFromVirtualBase"
);
forward_interface!(
    fc4p_00648220,
    0x0064_8220,
    "sub ecx, 484",
    "sub ecx, [ecx - 4]",
    0x0080_18A0,
    "SetModelStateFromVirtualBase"
);
forward_interface!(
    fc4p_00648240,
    0x0064_8240,
    "",
    "sub ecx, [ecx - 4]",
    0x0065_4140,
    "InitializeConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648250,
    0x0064_8250,
    "",
    "sub ecx, [ecx - 4]",
    0x0065_4500,
    "ReadConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648260,
    0x0064_8260,
    "",
    "sub ecx, [ecx - 4]",
    0x0065_4590,
    "WriteConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648290,
    0x0064_8290,
    "sub ecx, 484",
    "sub ecx, [ecx - 4]",
    0x0080_18E0,
    "GetModelFlagsFromVirtualBase"
);
forward_interface!(
    fc4p_006482b0,
    0x0064_82B0,
    "sub ecx, 484",
    "sub ecx, [ecx - 4]",
    0x0080_1E10,
    "CopyModelDataFromVirtualBase"
);
forward_interface!(
    fc4p_006485d0,
    0x0064_85D0,
    "sub ecx, 4",
    "",
    0x0065_A860,
    "AddRefBridgeFromSecondInterface"
);
forward_interface!(
    fc4p_006485e0,
    0x0064_85E0,
    "sub ecx, 4",
    "",
    0x0065_A870,
    "ReleaseBridgeFromSecondInterface"
);
forward_interface!(
    fc4p_00648600,
    0x0064_8600,
    "sub ecx, 8",
    "",
    0x0065_A860,
    "AddRefBridgeFromThirdInterface"
);
forward_interface!(
    fc4p_00648610,
    0x0064_8610,
    "sub ecx, 8",
    "",
    0x0065_A870,
    "ReleaseBridgeFromThirdInterface"
);
forward_interface!(
    fc4p_006491d0,
    0x0064_91D0,
    "add ecx, 8",
    "",
    0x005E_8DF0,
    "ReleaseOccupantAtEight"
);
forward_interface!(
    fc4p_0064dd70,
    0x0064_DD70,
    "sub ecx, 4",
    "",
    0x0064_91C0,
    "AddRefPropFromSecondInterface"
);
forward_interface!(
    fc4p_0064dd80,
    0x0064_DD80,
    "sub ecx, 4",
    "",
    0x0064_91D0,
    "ReleaseConstructionFromSecondInterface"
);
forward_interface!(
    fc4p_0064dd90,
    0x0064_DD90,
    "sub ecx, 8",
    "",
    0x0064_91C0,
    "AddRefPropFromThirdInterface"
);
forward_interface!(
    fc4p_0064dda0,
    0x0064_DDA0,
    "sub ecx, 8",
    "",
    0x0064_91D0,
    "ReleaseConstructionFromThirdInterface"
);
forward_interface!(
    fc4p_0064ddb0,
    0x0064_DDB0,
    "sub ecx, 12",
    "",
    0x0064_91C0,
    "AddRefPropFromFourthInterface"
);
forward_interface!(
    fc4p_0064ddc0,
    0x0064_DDC0,
    "sub ecx, 12",
    "",
    0x0064_91D0,
    "ReleaseConstructionFromFourthInterface"
);
forward_interface!(
    fc4p_0064ddd0,
    0x0064_DDD0,
    "sub ecx, 16",
    "",
    0x0064_91C0,
    "AddRefPropFromFifthInterface"
);
forward_interface!(
    fc4p_0064dde0,
    0x0064_DDE0,
    "sub ecx, 16",
    "",
    0x0064_91D0,
    "ReleaseConstructionFromFifthInterface"
);
forward_interface!(
    fc4p_00652e50,
    0x0065_2E50,
    "sub ecx, 4",
    "",
    0x0065_2CF0,
    "ShutdownPipeFromSecondInterface"
);
forward_interface!(
    fc4p_0068aff0,
    0x0068_AFF0,
    "add ecx, 4",
    "",
    0x006E_94C0,
    "CountPortsFromPreviousInterface"
);
forward_interface!(
    fc4p_0068b000,
    0x0068_B000,
    "add ecx, 4",
    "",
    0x006E_A2F0,
    "GetPortLotsFromPreviousInterface"
);
forward_interface!(
    fc4p_0068b1d0,
    0x0068_B1D0,
    "add ecx, 4",
    "",
    0x004F_7BA0,
    "AddRefAtNextInterface"
);
forward_interface!(
    fc4p_0068b1e0,
    0x0068_B1E0,
    "add ecx, 4",
    "",
    0x006E_8F90,
    "ReleasePortAtNextInterface"
);
forward_interface!(
    fc4p_0068d230,
    0x0068_D230,
    "sub ecx, 4",
    "",
    0x004F_7BA0,
    "AddRefFromSecondInterface"
);
forward_interface!(
    fc4p_0068d250,
    0x0068_D250,
    "sub ecx, 8",
    "",
    0x004F_7BA0,
    "AddRefFromThirdInterface"
);
forward_interface!(
    fc4p_006b8ab0,
    0x006B_8AB0,
    "sub ecx, 4",
    "",
    0x006D_5140,
    "ReleaseLandfillFromSecondInterface"
);
forward_interface!(
    fc4p_006b8ac0,
    0x006B_8AC0,
    "sub ecx, 8",
    "",
    0x006D_5140,
    "ReleaseLandfillFromThirdInterface"
);

port! {
    /// Returns the caller's argument word and removes it from the stack.
    #[port(address = 0x0064_8F30, owner = "shared", name = "ReturnArgumentWord", naked)]
    pub unsafe extern "C" fn fc4p_00648f30() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "ret 4");
    }
}

macro_rules! constant_entry {
    ($export:ident, $address:literal, $value:literal, $name:literal) => {
        port! {
            /// Returns the fixed class identifier, leaving the other registers unchanged.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("mov eax, {value}", "ret", value = const $value);
            }
        }
    };
}
constant_entry!(fc4p_00648f40, 0x0064_8F40, 0x49C0_5B9Fu32, "GetPipeClassId");
constant_entry!(fc4p_00653d90, 0x0065_3D90, 0xCA16_374Fu32, "GetSubwayClassId");
constant_entry!(fc4p_0068afd0, 0x0068_AFD0, 0xAA20_49A1u32, "GetAirportClassId");
constant_entry!(fc4p_006b8b10, 0x006B_8B10, 0xEA09_D4AEu32, "GetLandfillClassId");

port! {
    /// Destroys the underground occupant and frees it when the low flag bit is set.
    #[port(address = 0x0064_8F50, owner = "shared", name = "DeleteUndergroundOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00648f50(this: *mut u8, flags: u32) -> u64 {
        unsafe {
            let mut result = underground_destroy(this);
            if flags & 1 != 0 { result = free_object(this); }
            (result & !0xFFFF_FFFF) | u64::from(this as u32)
        }
    }
}

port! {
    /// Removes three stack words without changing registers or memory.
    #[port(address = 0x0065_7730, owner = "shared", name = "IgnoreThreeArguments", naked)]
    pub unsafe extern "C" fn fc4p_00657730() { core::arch::naked_asm!("ret 12"); }
}

port! {
    /// Stores the low argument byte as the resource factory's write mode, retaining EAX's upper bytes.
    #[port(address = 0x0067_CB10, owner = "shared", name = "SetResourceWriteMode", naked)]
    pub unsafe extern "C" fn fc4p_0067cb10() {
        core::arch::naked_asm!("mov al, [esp + 4]", "mov [ecx + 0x10], al", "ret 4");
    }
}

port! {
    /// Tests whether the low argument byte is 2 or 3.
    #[port(address = 0x0069_5260, owner = "shared", name = "IsMiddlePair", naked)]
    pub unsafe extern "C" fn fc4p_00695260() {
        core::arch::naked_asm!("push ecx", "push edx", "push [esp + 12]", "call {body}",
            "add esp, 4", "pop edx", "pop ecx", "ret", body = sym is_middle_pair);
    }
}

extern "C" fn is_middle_pair(value: u32) -> u32 {
    u32::from(matches!(value as u8, 2 | 3))
}

port! {
    /// Increments the reference count at offset 0x14, with 32-bit wrapping.
    #[port(address = 0x0069_5330, owner = "shared", name = "AddRefAtTwenty")]
    pub unsafe extern "thiscall" fn fc4p_00695330(this: *mut u8) -> u32 {
        unsafe {
            let count = word(this, 0x14).wrapping_add(1);
            put(this, 0x14, count);
            count
        }
    }
}

port! {
    /// Returns the stored monthly consumption word without changing other registers.
    #[port(address = 0x006A_AEB0, owner = "shared", name = "GetMonthlyConsumption", naked)]
    pub unsafe extern "C" fn fc4p_006aaeb0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x54]", "ret");
    }
}

port! {
    /// Returns the stored borrowing total in EDX:EAX.
    #[port(address = 0x006B_9850, owner = "shared", name = "GetTotalBorrowed", naked)]
    pub unsafe extern "C" fn fc4p_006b9850() {
        core::arch::naked_asm!("mov eax, [ecx + 0x38]", "mov edx, [ecx + 0x3c]", "ret");
    }
}

game_fn! { fn free_storage(storage: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { fn destroy_storage_subtree(this: *mut u8, root: *mut u8) = "thiscall" 0x0063_D700; }
game_fn! { fn allocate_storage(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn initialize_delegate(this: *mut u8) = "thiscall" 0x0090_D957; }
game_fn! { fn set_resource_key(this: *mut u8, key: *mut u8) -> u64 = "thiscall" 0x0097_5402; }
game_fn! { fn get_resource_key(this: *mut u8, output: *mut u32) -> *mut u8 = "thiscall" 0x0097_53EA; }
game_fn! { fn lower_bound_pair(first: *mut u8, last: *mut u8, key: *const u32, value: u32, cookie: u32) -> u64 = "C" 0x005F_E940; }
game_fn! { fn allocate_object(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn construct_collection(this: *mut u8) -> *mut u8 = "thiscall" 0x0090_DC0F; }
game_fn! { fn serialize_range(stream: *mut u8, range: *mut u8) = "C" 0x005D_AFF0; }
game_fn! { fn find_first_occupant(x: u32, z: u32, output: *mut *mut u8, service: *mut u8) -> u64 = "C" 0x0065_84D0; }
game_fn! { fn find_second_occupant(x: u32, z: u32, service: *mut u8, output: *mut *mut u8) -> u64 = "C" 0x0065_85E0; }
game_fn! { #[allow(dead_code)] fn initialize_scaled_object(this: *mut u8) -> u64 = "thiscall" 0x006F_6A70; }

layout! {
    /// Storage fields released during network-tool destruction.
    struct ToolStorage {
        0x14 => first_buffer / set_first_buffer: *mut u8,
        0x20 => second_buffer / set_second_buffer: *mut u8,
        0x44 => third_buffer / set_third_buffer: *mut u8,
        0x50 => fourth_buffer / set_fourth_buffer: *mut u8,
        0x60 => fifth_buffer / set_fifth_buffer: *mut u8,
        0x6C => sixth_buffer / set_sixth_buffer: *mut u8,
        0x80 => seventh_buffer / set_seventh_buffer: *mut u8,
        0x8C => eighth_buffer / set_eighth_buffer: *mut u8,
        0x158 => string_table / set_string_table: u32,
        0x15C => string_buffer / set_string_buffer: *mut u8,
        0x16C => tree_header / set_tree_header: *mut u8,
        0x170 => tree_count / set_tree_count: u32,
    }
}

layout! {
    /// The three links in a tree sentinel.
    struct TreeHeader {
        0x04 => root / set_root: *mut u8,
        0x08 => first / set_first: *mut u8,
        0x0C => last / set_last: *mut u8,
    }
}

port! {
    /// Clears a nonempty tree, frees its sentinel, restores the string table, then frees owned buffers.
    /// Freed pointer fields retain their original values.
    #[port(address = 0x0064_0230, owner = "shared", name = "DestroyToolStorage")]
    pub unsafe extern "fastcall" fn fc4p_00640230(this: *mut u8, entry_edx: u32) -> u64 {
        unsafe {
            let storage = ToolStorage::from_ptr(this);
            let mut result = registers(storage.tree_count(), entry_edx);
            if storage.tree_count() != 0 {
                destroy_storage_subtree(this.add(0x16C), TreeHeader::from_ptr(storage.tree_header()).root());
                let header = storage.tree_header();
                TreeHeader::from_ptr(header).set_first(header);
                let header = storage.tree_header();
                TreeHeader::from_ptr(header).set_root(core::ptr::null_mut());
                let last = storage.tree_header();
                TreeHeader::from_ptr(last).set_last(last);
                storage.set_tree_count(0);
                result = registers(last as u32, header as u32);
            }
            let header = storage.tree_header();
            if !header.is_null() { result = free_storage(header); }
            storage.set_string_table(0x00A8_0810);
            for offset in [0x15C, 0x8C, 0x80, 0x6C, 0x60, 0x50, 0x44, 0x20] {
                let p = pointer(this, offset);
                result = registers(p as u32, (result >> 32) as u32);
                if !p.is_null() { result = free_storage(p); }
            }
            let p = storage.first_buffer();
            if !p.is_null() { result = free_storage(p); }
            result
        }
    }
}

layout! {
    /// The owned fields initialized by the shared tool helper constructor; gaps remain untouched.
    struct ToolHelper {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => secondary_table / set_secondary_table: u32,
        0x08 => third_table / set_third_table: u32,
        0x0C => delegate_table / set_delegate_table: u32,
        0x14 => field14 / set_field14: u32,
        0x18 => field18 / set_field18: u32,
        0x24 => flag24 / set_flag24: u8,
        0x25 => flag25 / set_flag25: u8,
        0x26 => flag26 / set_flag26: u8,
        0x27 => flag27 / set_flag27: u8,
        0x29 => flag29 / set_flag29: u8,
        0x2A => flag2a / set_flag2a: u8,
        0x2B => flag2b / set_flag2b: u8,
        0x2C => field2c / set_field2c: u32,
        0x30 => field30 / set_field30: u32,
        0x44 => field44 / set_field44: u32,
        0x48 => field48 / set_field48: u32,
        0x4C => field4c / set_field4c: u32,
        0x50 => string_table / set_string_table: u32,
        0x54 => string_first / set_string_first: *mut u8,
        0x58 => string_last / set_string_last: *mut u8,
        0x5C => string_limit / set_string_limit: *mut u8,
        0x60 => field60 / set_field60: u32,
        0x74 => field74 / set_field74: u32,
        0x78 => field78 / set_field78: u32,
        0x7C => field7c / set_field7c: u32,
        0xAC => fieldac / set_fieldac: u32,
        0xB0 => fieldb0 / set_fieldb0: u32,
        0xB4 => fieldb4 / set_fieldb4: u32,
        0xD8 => fieldd8 / set_fieldd8: u32,
        0xDC => fielddc / set_fielddc: u32,
        0xE0 => fielde0 / set_fielde0: u32,
        0xE4 => fielde4 / set_fielde4: u32,
        0xE8 => fielde8 / set_fielde8: u32,
        0xEC => fieldec / set_fieldec: u32,
        0xF0 => fieldf0 / set_fieldf0: u32,
        0xF4 => fieldf4 / set_fieldf4: u32,
        0xF8 => fieldf8 / set_fieldf8: u32,
        0x124 => field124 / set_field124: u32,
        0x128 => field128 / set_field128: u32,
        0x12C => field12c / set_field12c: u32,
        0x130 => field130 / set_field130: u32,
    }
}

port! {
    /// Initializes the delegate before installing the final interface tables and allocating an empty string.
    #[port(address = 0x0064_5D50, owner = "shared", name = "ConstructToolHelper")]
    pub unsafe extern "thiscall" fn fc4p_00645d50(this: *mut u8) -> *mut u8 {
        unsafe {
            let object = ToolHelper::from_ptr(this);
            object.set_primary_table(0x00AA_90C0);
            object.set_secondary_table(0x00A8_3524);
            object.set_third_table(0x00A8_1174);
            initialize_delegate(this.add(0x0C));
            object.set_primary_table(0x00AA_9140);
            object.set_secondary_table(0x00AA_9128);
            object.set_third_table(0x00AA_9118);
            object.set_delegate_table(0x00AA_9104);
            for offset in [0x14, 0x18] { put(this, offset, 0); }
            for offset in [0x24, 0x25, 0x26, 0x27, 0x29, 0x2A, 0x2B] { this.add(offset).write(0); }
            for offset in [0x2C, 0x30, 0x44, 0x48, 0x4C, 0x54, 0x58, 0x5C] { put(this, offset, 0); }
            let buffer = allocate_storage(8);
            object.set_string_first(buffer);
            object.set_string_last(buffer);
            object.set_string_limit(buffer.wrapping_add(8));
            buffer.write(0);
            object.set_field60(0);
            object.set_string_table(0x00A8_0810);
            for offset in [0x74, 0x78, 0x7C, 0xAC, 0xB0, 0xB4, 0xD8, 0xDC, 0xE0,
                0xE4, 0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0x124, 0x128, 0x12C, 0x130] { put(this, offset, 0); }
            this
        }
    }
}

port! {
    /// Copies eight-byte records backward, with a signed count; returns the destination start and displacement.
    #[port(address = 0x0066_B340, owner = "shared", name = "CopyEightByteRecordsBackward")]
    pub unsafe extern "C" fn fc4p_0066b340(first: *mut u8, last: *mut u8, output: *mut u8) -> u64 {
        unsafe {
            let count = (last as u32).wrapping_sub(first as u32) as i32 >> 3;
            let mut output = output;
            let mut source = last;
            if count <= 0 { return registers(output as u32, last as u32); }
            let displacement = (last as u32).wrapping_sub(output as u32);
            for _ in 0..count {
                source = source.wrapping_sub(8);
                let low = word(source, 0);
                output = output.wrapping_sub(8);
                put(output, 0, low);
                put(output, 4, word(source, 4));
            }
            registers(output as u32, displacement)
        }
    }
}

port! {
    /// Sets the key on the embedded resource interface and replaces only AL with one.
    #[port(address = 0x0067_C220, owner = "shared", name = "SetEmbeddedResourceKey")]
    pub unsafe extern "thiscall" fn fc4p_0067c220(this: *mut u8, key: *mut u8) -> u64 {
        unsafe { (set_resource_key(this.add(4), key) & !0xFF) | 1 }
    }
}

layout! {
    /// Sorted pairs used by the shared resource lookup helpers.
    struct ResourcePairs {
        0x18 => first / set_first: *mut u8,
        0x1C => last / set_last: *mut u8,
    }
}

unsafe fn find_resource_pair(this: *mut u8, key: u32) -> (u64, bool) {
    unsafe {
        let pairs = ResourcePairs::from_ptr(this);
        let last = pairs.last();
        let value = [key, 0];
        let result = lower_bound_pair(pairs.first(), last, value.as_ptr(), key, 0);
        let candidate = result as u32 as *mut u8;
        (result, candidate != last && word(candidate, 0) == key)
    }
}

port! {
    /// Checks the key returned by the original pair lower-bound helper, retaining its upper return bytes.
    #[port(address = 0x0067_C620, owner = "shared", name = "ContainsResourcePair")]
    pub unsafe extern "thiscall" fn fc4p_0067c620(this: *mut u8, key: u32) -> u64 {
        let (result, found) = unsafe { find_resource_pair(this, key) };
        (result & !0xFF) | u64::from(found)
    }
}

port! {
    /// Returns the matching pair's value or zero, retaining the lower-bound helper's EDX.
    #[port(address = 0x0067_C670, owner = "shared", name = "GetResourcePairValue")]
    pub unsafe extern "thiscall" fn fc4p_0067c670(this: *mut u8, key: u32) -> u64 {
        unsafe {
            let (result, found) = find_resource_pair(this, key);
            let value = if found { word(result as u32 as *mut u8, 4) } else { 0 };
            registers(value, (result >> 32) as u32)
        }
    }
}

port! {
    /// Copies the embedded interface's three-word key in word order, then sets AL to one.
    #[port(address = 0x0067_CD20, owner = "shared", name = "GetEmbeddedResourceKey")]
    pub unsafe extern "thiscall" fn fc4p_0067cd20(this: *mut u8, output: *mut u8) -> u64 {
        unsafe {
            let mut temporary = core::mem::MaybeUninit::<[u32; 3]>::uninit();
            let key = get_resource_key(this.add(4), temporary.as_mut_ptr().cast());
            put(output, 0, word(key, 0));
            let middle = word(key, 4);
            put(output, 4, middle);
            let last = word(key, 8);
            put(output, 8, last);
            registers((last & !0xFF) | 1, middle)
        }
    }
}

port! {
    /// Constructs a retained collection and appends each pair's value. A null output only clears AL.
    #[port(address = 0x0067_C4B0, owner = "shared", name = "CollectResourcePairValues", naked)]
    pub unsafe extern "C" fn fc4p_0067c4b0() {
        core::arch::naked_asm!("push edx", "push eax", "mov edx, esp", "call {body}", "add esp, 8", "ret 4",
            body = sym collect_resource_pair_values);
    }
}

unsafe extern "fastcall" fn collect_resource_pair_values(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let output = frame.add(3).read() as *mut *mut u8;
        if output.is_null() {
            return registers(frame.read() & !0xFF, frame.add(1).read());
        }
        let allocation = allocate_object(12);
        let collection = if allocation.is_null() {
            allocation
        } else {
            construct_collection(allocation)
        };
        let mut result = vcall!(collection, 4 => fn() -> u64);
        let pairs = ResourcePairs::from_ptr(this);
        let (mut first, last) = (pairs.first(), pairs.last());
        while first != last {
            result = vcall!(collection, 0x1C => fn(u32) -> u64, word(first, 4));
            first = first.wrapping_add(8);
        }
        output.write_unaligned(collection);
        (result & !0xFF) | 1
    }
}

port! {
    /// Obtains service number one and forwards to its +0x2C method.
    #[port(address = 0x0068_B270, owner = "shared", name = "CallFirstServiceMethod")]
    pub unsafe extern "thiscall" fn fc4p_0068b270(this: *mut u8) -> u64 {
        unsafe {
            let service = vcall!(pointer(this, 0x30), 0x50 => fn(u32) -> *mut u8, 1);
            vcall!(service, 0x2C => fn() -> u64)
        }
    }
}

port! {
    /// Writes version one, the owned range and its trailing word; success means a zero low error byte.
    #[port(address = 0x006A_C930, owner = "shared", name = "WriteRangeRecord")]
    pub unsafe extern "thiscall" fn fc4p_006ac930(this: *mut u8, stream: *mut u8) -> u64 {
        unsafe {
            vcall!(stream, 0x1C => fn(u32) -> u32, 1);
            serialize_range(stream, this.add(0x14));
            vcall!(stream, 0x24 => fn(u32) -> u32, word(this, 0x1C));
            let result = vcall!(stream, 0x48 => fn() -> u64);
            registers(u32::from(result as u8 == 0), (result >> 32) as u32)
        }
    }
}

port! {
    /// Restores unit scale, then forwards the original frame to the initializer.
    #[port(address = 0x006A_CF70, owner = "shared", name = "InitializeWithUnitScale", naked)]
    pub unsafe extern "C" fn fc4p_006acf70() {
        #[cfg(not(test))]
        core::arch::naked_asm!("mov dword ptr [ecx + 0x98], 0x3f800000", "jmp dword ptr [2f]", "2: .long 0x006f6a70");
        #[cfg(test)]
        core::arch::naked_asm!("mov dword ptr [ecx + 0x98], 0x3f800000",
            "push eax", "push ecx", "push edx", "push 0x006f6a70", "call {resolve}", "add esp, 4",
            "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret", resolve = sym forwarding_target);
    }
}

port! {
    /// Checks two occupant searches in order, releasing the first successful result.
    #[port(address = 0x0065_9470, owner = "shared", name = "HasEitherOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00659470(this: *mut u8, x: u32, z: u32) -> u64 {
        unsafe {
            let (mut first, mut second) = (core::ptr::null_mut(), core::ptr::null_mut());
            let service = vcall!(this, 0x144 => fn() -> *mut u8);
            let result = find_first_occupant(x, z, &mut first, service);
            if result as u8 != 0 { return (vcall!(first, 8 => fn() -> u64) & !0xFF) | 1; }
            let service = vcall!(this, 0x144 => fn() -> *mut u8);
            let result = find_second_occupant(x, z, service, &mut second);
            if result as u8 != 0 { return (vcall!(second, 8 => fn() -> u64) & !0xFF) | 1; }
            result & !0xFF
        }
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00637e20::PORT,
    fc4p_00637e60::PORT,
    fc4p_006385d0::PORT,
    fc4p_00638600::PORT,
    fc4p_00638650::PORT,
    fc4p_00638710::PORT,
    fc4p_0063c020::PORT,
    fc4p_0063c070::PORT,
    fc4p_0063e9d0::PORT,
    fc4p_00640230::PORT,
    fc4p_00645d50::PORT,
    fc4p_006480d0::PORT,
    fc4p_006480e0::PORT,
    fc4p_006481d0::PORT,
    fc4p_006481e0::PORT,
    fc4p_00648200::PORT,
    fc4p_00648210::PORT,
    fc4p_00648220::PORT,
    fc4p_00648240::PORT,
    fc4p_00648250::PORT,
    fc4p_00648260::PORT,
    fc4p_00648290::PORT,
    fc4p_006482b0::PORT,
    fc4p_006485d0::PORT,
    fc4p_006485e0::PORT,
    fc4p_00648600::PORT,
    fc4p_00648610::PORT,
    fc4p_00648f30::PORT,
    fc4p_00648f40::PORT,
    fc4p_00648f50::PORT,
    fc4p_006491d0::PORT,
    fc4p_0064dd70::PORT,
    fc4p_0064dd80::PORT,
    fc4p_0064dd90::PORT,
    fc4p_0064dda0::PORT,
    fc4p_0064ddb0::PORT,
    fc4p_0064ddc0::PORT,
    fc4p_0064ddd0::PORT,
    fc4p_0064dde0::PORT,
    fc4p_00652e50::PORT,
    fc4p_00653d90::PORT,
    fc4p_00657730::PORT,
    fc4p_00659470::PORT,
    fc4p_0066b340::PORT,
    fc4p_0067c220::PORT,
    fc4p_0067c4b0::PORT,
    fc4p_0067c620::PORT,
    fc4p_0067c670::PORT,
    fc4p_0067cb10::PORT,
    fc4p_0067cd20::PORT,
    fc4p_0068afd0::PORT,
    fc4p_0068aff0::PORT,
    fc4p_0068b000::PORT,
    fc4p_0068b1d0::PORT,
    fc4p_0068b1e0::PORT,
    fc4p_0068b270::PORT,
    fc4p_0068d230::PORT,
    fc4p_0068d250::PORT,
    fc4p_00695260::PORT,
    fc4p_00695330::PORT,
    fc4p_006aaeb0::PORT,
    fc4p_006ac930::PORT,
    fc4p_006acf70::PORT,
    fc4p_006b8ab0::PORT,
    fc4p_006b8ac0::PORT,
    fc4p_006b8b10::PORT,
    fc4p_006b9850::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{Conv, RawCall};
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq, Eq)]
    struct Call(u32, usize, Vec<u32>);

    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        query_result: u32,
        interface: usize,
        active: u32,
        first: u32,
        second: u32,
        cell: usize,
        kind: u32,
        mutate: Option<(usize, usize, u32)>,
    }

    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    fn log(id: u32, this: *mut u8, args: &[u32]) {
        STATE.with(|s| s.borrow_mut().calls.push(Call(id, this as usize, args.to_vec())));
    }

    fn calls() -> Vec<Call> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }

    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }

    unsafe fn run(entry: *const (), conv: Conv, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> (u32, u32) {
        let result = unsafe { run_registers(entry, conv, this, args, eax, edx) };
        (result[0], result[2])
    }

    fn environment() -> (u16, u16, u32, u32) {
        let (mut cw, mut mxcsr) = (0u16, 0u32);
        let (sw, flags): (u16, u32);
        unsafe {
            core::arch::asm!("fnstcw [{cw}]", "fnstsw ax", "stmxcsr [{mxcsr}]", "pushfd", "pop {flags:e}",
                cw = in(reg) &mut cw, mxcsr = in(reg) &mut mxcsr, flags = lateout(reg) flags, out("ax") sw);
        }
        (cw, sw & 0x3800, mxcsr, flags & 0x400)
    }

    unsafe fn run_registers(entry: *const (), conv: Conv, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> [u32; 3] {
        #[repr(C)]
        struct Probe {
            call: RawCall,
            saved: [u32; 6],
        }
        let mut call = RawCall::new(conv, entry as usize, this as u32, args);
        call.eax = eax;
        call.edx = edx;
        let mut probe = Probe { call, saved: [0; 6] };
        let before = environment();
        let sentinel = 123.25f64;
        let mut restored = 0.0f64;
        unsafe {
            core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
        }
        // The probe records each preserved register and ESP around the original argument frame.
        unsafe {
            core::arch::asm!(
                "push ebx", "push esi", "push ebp", "mov ebx, 0x12345678", "mov esi, 0x9abcdef0",
                "mov ebp, 0x76543210", "mov [edi + 64], esp", "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
                "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push [eax + ecx*4]", "jmp 2b",
                "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]", "call [edi]",
                "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]",
                "4:", "mov [edi + 44], ebx", "mov [edi + 48], esi", "mov [edi + 52], ebp",
                "mov [edi + 56], edi", "mov [edi + 60], esp", "pop ebp", "pop esi", "pop ebx",
                in("edi") &mut probe, clobber_abi("C"),
            );
        }
        unsafe {
            core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
        }
        assert_eq!(restored.to_bits(), sentinel.to_bits());
        assert_eq!(environment(), before);
        assert_eq!(probe.saved[..3], [0x1234_5678, 0x9ABC_DEF0, 0x7654_3210]);
        assert_eq!(probe.saved[3], (&probe as *const Probe) as u32);
        assert_eq!(probe.saved[4], probe.saved[5]);
        [probe.call.eax, probe.call.ecx, probe.call.edx]
    }

    fn bytes(p: *mut u8, length: usize) -> Vec<u8> {
        unsafe { core::slice::from_raw_parts(p, length).to_vec() }
    }

    extern "thiscall" fn query(this: *mut u8, iid: u32, output: *mut *mut u8) -> u64 {
        log(0, this, &[iid]);
        STATE.with(|s| {
            let s = s.borrow();
            unsafe {
                output.write(s.interface as *mut u8);
            }
            registers(s.query_result, 0x0011_2233)
        })
    }

    extern "thiscall" fn active(this: *mut u8) -> u64 {
        log(0x54, this, &[]);
        STATE.with(|s| registers(s.borrow().active, 0x0044_5566))
    }

    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(8, this, &[]);
        registers(0xA1B2_C3D4, 0x0077_8899)
    }

    extern "thiscall" fn delegate(this: *mut u8, object: *mut u8) -> u64 {
        log(0x0C, this, &[object as u32]);
        STATE.with(|s| registers(s.borrow().first, 0x00AA_BBCC))
    }

    extern "C" fn check_active(object: *mut u8) -> u64 {
        log(0x0063_8600, object, &[]);
        STATE.with(|s| registers(s.borrow().active, 0x00DD_EEFF))
    }

    extern "thiscall" fn destroy(this: *mut u8) -> u64 {
        log(0x005E_95C0, this, &[]);
        registers(0x1020_3040, 0x005E_95C0)
    }
    extern "thiscall" fn destroy_underground(this: *mut u8) -> u64 {
        log(0x0064_8E90, this, &[]);
        registers(0x5060_7080, 0x0064_8E90)
    }
    extern "C" fn free(this: *mut u8) -> u64 {
        log(0x005E_5620, this, &[]);
        registers(0x90A0_B0C0, 0x005E_5620)
    }

    extern "thiscall" fn first(this: *mut u8) -> u64 {
        log(0xFC, this, &[]);
        STATE.with(|s| {
            let s = s.borrow();
            if let Some((p, offset, value)) = s.mutate {
                unsafe {
                    put(p as *mut u8, offset, value);
                }
            }
            registers(s.first, 0x00FC_0011)
        })
    }

    extern "thiscall" fn second(this: *mut u8, x: u32, z: u32) -> u64 {
        log(0x88, this, &[x, z]);
        STATE.with(|s| registers(s.borrow().second, 0x0088_2233))
    }

    extern "thiscall" fn cell(this: *mut u8, key: u32) -> u64 {
        log(0x0063_3220, this, &[key]);
        STATE.with(|s| registers(s.borrow().cell as u32, 0x0033_4455))
    }

    extern "thiscall" fn kind_body(this: *mut u8) -> u32 {
        log(0x0063_DDF0, this, &[]);
        STATE.with(|s| {
            let s = s.borrow();
            if let Some((p, offset, value)) = s.mutate {
                unsafe {
                    put(p as *mut u8, offset, value);
                }
            }
            s.kind
        })
    }

    #[unsafe(naked)]
    unsafe extern "C" fn kind() {
        core::arch::naked_asm!("call {body}", "mov ecx, 0x11223344", "mov edx, 0x55667788", "ret", body = sym kind_body);
    }

    #[test]
    fn copy_records_keeps_word_order_for_overlapping_ranges_and_returns_both_ends() {
        unsafe {
            let mut data = [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12];
            let p = data.as_mut_ptr().cast::<u8>();
            let result = run(
                fc4p_00637e20 as *const (),
                Conv::Cdecl,
                core::ptr::null_mut(),
                &[p as u32, p.add(32) as u32, p.add(4) as u32],
                0x1122_3344,
                0x5566_7788,
            );
            assert_eq!(data, [1, 1, 1, 1, 1, 1, 1, 1, 1, 10, 11, 12]);
            assert_eq!(result, (p.add(36) as u32, p.add(32) as u32));
            assert_eq!(
                fc4p_00637e20(p, p, p.add(5)),
                (u64::from(p as u32) << 32) | u64::from(p.add(5) as u32)
            );
            assert_eq!(
                fc4p_00637e20(core::ptr::null(), core::ptr::null(), core::ptr::null_mut()),
                0
            );
            // The first destination can be null without reading an inaccessible source.
            assert_eq!(
                fc4p_00637e20(0x1000 as *const u8, 0x1010 as *const u8, core::ptr::null_mut()),
                0x1010_0000_0010
            );
            let source = [0xAABB_CCDDu32; 8];
            let mut output = [0x1234_5678u32; 10];
            let a = source.as_ptr().cast::<u8>();
            let b = output.as_mut_ptr().cast::<u8>();
            fc4p_00637e20(a, a.add(32), b.add(4));
            assert_eq!(
                output,
                [
                    0x1234_5678,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0xAABB_CCDD,
                    0x1234_5678
                ]
            );
        }
    }

    #[test]
    fn fill_records_reloads_an_aliased_value_and_leaves_guards_untouched() {
        unsafe {
            let mut data = [1u32, 2, 3, 4, 5, 6, 7, 8];
            let p = data.as_mut_ptr().cast::<u8>();
            assert_eq!(fc4p_00637e60(p.add(4), p.add(28), p), p.add(28));
            assert_eq!(data, [1, 1, 1, 1, 1, 1, 1, 8]);
            assert_eq!(fc4p_00637e60(p, p, core::ptr::null()), p);
            assert_eq!(
                fc4p_00637e60(core::ptr::null_mut(), core::ptr::null_mut(), core::ptr::null()),
                core::ptr::null_mut()
            );
            let value = [0x1234_5678u32, 0, u32::MAX];
            fc4p_00637e60(p.add(4), p.add(28), value.as_ptr().cast());
            assert_eq!(data, [1, 0x1234_5678, 0, u32::MAX, 0x1234_5678, 0, u32::MAX, 8]);
        }
    }

    #[test]
    fn active_query_uses_low_bytes_and_releases_even_an_interface_returned_on_failure() {
        reset();
        let mut vt = [0usize; 0x58 / 4];
        vt[0] = query as *const () as usize;
        vt[2] = release as *const () as usize;
        vt[0x54 / 4] = active as *const () as usize;
        let mut object = [vt.as_ptr() as usize];
        let mut interface = [vt.as_ptr() as usize];
        let p = object.as_mut_ptr().cast::<u8>();
        let n = interface.as_mut_ptr().cast::<u8>();
        unsafe {
            assert_eq!(
                run(
                    fc4p_00638600 as *const (),
                    Conv::Cdecl,
                    core::ptr::null_mut(),
                    &[0],
                    0,
                    0x1234_5678
                ),
                (0, 0x1234_5678)
            );
        }
        assert!(calls().is_empty());
        for (qi, a, has_interface, result, checks) in [
            (0xABCD_0000, 0xFF, false, 0, false),
            (0xABCD_0000, 0xFF, true, 0xA1B2_C300, false),
            (0xABCD_0080, 0x5566_7700, true, 0xA1B2_C300, true),
            (0xABCD_0080, 0x5566_7780, true, 0xA1B2_C301, true),
        ] {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.query_result = qi;
                s.active = a;
                s.interface = if has_interface { n as usize } else { 0 };
            });
            let edx = if has_interface { 0x0077_8899 } else { 0x0011_2233 };
            unsafe {
                assert_eq!(
                    run(
                        fc4p_00638600 as *const (),
                        Conv::Cdecl,
                        core::ptr::null_mut(),
                        &[p as u32],
                        0,
                        0
                    ),
                    (result, edx)
                );
            }
            let mut expected = vec![Call(0, p as usize, vec![0xA821_EF94])];
            if checks {
                expected.push(Call(0x54, n as usize, vec![]));
            }
            if has_interface {
                expected.push(Call(8, n as usize, vec![]));
            }
            assert_eq!(calls(), expected);
        }
    }

    #[test]
    fn delegate_checks_only_after_success_and_calls_the_original_active_helper() {
        reset();
        let _fake = fake(0x0063_8600, check_active as *const () as usize);
        let vt = [0usize, 0, 0, delegate as *const () as usize];
        let mut d = [vt.as_ptr() as usize];
        let dp = d.as_mut_ptr().cast::<u8>();
        let mut object = [0usize, 0, 0, dp as usize];
        let p = object.as_mut_ptr().cast::<u8>();
        let target = 0x1234_5678 as *mut u8;
        for (a, b, result) in [(0x1234_0000, 1, 0), (0x1234_0080, 0x5500, 0), (0x1234_0080, 0x5580, 1)] {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.first = a;
                s.active = b;
            });
            unsafe {
                assert_eq!(
                    fc4p_006385d0(p, target),
                    registers(result, if a as u8 == 0 { 0x00AA_BBCC } else { 0x00DD_EEFF })
                );
            }
            let mut expected = vec![Call(0x0C, dp as usize, vec![target as u32])];
            if a as u8 != 0 {
                expected.push(Call(0x0063_8600, target as usize, vec![]));
            }
            assert_eq!(calls(), expected);
        }
    }

    #[test]
    fn deleting_entries_use_the_original_destroy_and_free_calls_in_order() {
        reset();
        let _base = fake(0x005E_95C0, destroy as *const () as usize);
        let _underground = fake(0x0064_8E90, destroy_underground as *const () as usize);
        let _free = fake(0x005E_5620, free as *const () as usize);
        let vt = [0usize, 0, release as *const () as usize];
        let mut retained = [vt.as_ptr() as usize];
        let d = retained.as_mut_ptr().cast::<u8>();
        let mut object = [0xAAAAAAAAusize; 8];
        let p = object.as_mut_ptr().cast::<u8>();
        for entry in [fc4p_00638650 as *const (), fc4p_00648f50 as *const ()] {
            for retained in [0usize, d as usize] {
                object[3] = retained;
                for flags in [0, 1, 2, 3, 0x100, 0xFFFF_FFFE, u32::MAX] {
                    let before = bytes(p, 32);
                    let result = unsafe { run(entry, Conv::Thiscall, p, &[flags], 0, 0) };
                    assert_eq!(result.0, p as u32);
                    assert_eq!(bytes(p, 32), before);
                    let mut expected = vec![];
                    let base = if entry == fc4p_00638650 as *const () {
                        if retained != 0 {
                            expected.push(Call(8, retained, vec![]));
                        }
                        0x005E_95C0
                    } else {
                        0x0064_8E90
                    };
                    expected.push(Call(base, p as usize, vec![]));
                    if flags & 1 != 0 {
                        expected.push(Call(0x005E_5620, p as usize, vec![]));
                    }
                    assert_eq!(result.1, if flags & 1 != 0 { 0x005E_5620 } else { base });
                    assert_eq!(calls(), expected);
                }
            }
        }
    }

    #[test]
    fn network_cell_check_covers_short_circuits_rejection_and_pointer_reloads() {
        reset();
        let _lookup = fake(0x0063_3220, cell as *const () as usize);
        let mut vt = [0usize; 0x100 / 4];
        vt[0xFC / 4] = first as *const () as usize;
        let mut terrain_vt = [0usize; 0x8C / 4];
        terrain_vt[0x88 / 4] = second as *const () as usize;
        let mut terrain = [terrain_vt.as_ptr() as usize];
        let t = terrain.as_mut_ptr().cast::<u8>();
        let mut object = Object::new(0x70);
        let p = object.ptr();
        let mut coordinates = [u32::MAX, 0xFFFF_8000];
        let mut cell = [0u8; 0x60];
        let c = cell.as_mut_ptr();
        let mut output = 0xAABB_CCDDu32;
        unsafe {
            put(p, 0, vt.as_ptr() as u32);
            put(p, 0x44, t as u32);
            put(p, 0x60, coordinates.as_mut_ptr() as u32);
        }
        for (a, b, found, flag, rejected) in [
            (0x1234_5600, 1, true, 0, false),
            (0x1234_5680, 0xAABB_CC00, true, 0, false),
            (1, 0xAABB_CC80, false, 0, true),
            (1, 1, true, 0, true),
            (1, 1, true, 0x80, false),
        ] {
            cell[0x56] = flag;
            output = output.wrapping_add(1);
            let before = output;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.first = a;
                s.second = b;
                s.cell = if found { c as usize } else { 0 };
            });
            let result = unsafe { fc4p_00638710(p, &mut output) };
            let high = if rejected {
                (&mut output as *mut u32) as u32
            } else if a as u8 == 0 {
                a
            } else if b as u8 == 0 {
                b
            } else {
                c as u32
            };
            let edx = if a as u8 == 0 {
                0x00FC_0011
            } else if b as u8 == 0 {
                0x0088_2233
            } else {
                0x0033_4455
            };
            assert_eq!(result, registers((high & !0xFF) | u32::from(!rejected), edx));
            assert_eq!(output, if rejected { 0 } else { before });
            let mut expected = vec![Call(0xFC, p as usize, vec![])];
            if a as u8 != 0 {
                expected.push(Call(0x88, t as usize, coordinates.to_vec()));
                if b as u8 != 0 {
                    expected.push(Call(0x0063_3220, p as usize, vec![0x7FFF_FFFF]));
                }
            }
            assert_eq!(calls(), expected);
        }
        let mut next = [7u32, 11];
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.first = 1;
            s.second = 0;
            s.mutate = Some((p as usize, 0x60, next.as_mut_ptr() as u32));
        });
        unsafe {
            fc4p_00638710(p, &mut output);
        }
        assert_eq!(
            calls(),
            [Call(0xFC, p as usize, vec![]), Call(0x88, t as usize, vec![7, 11])]
        );
    }

    #[test]
    fn byte_range_equality_handles_empty_length_differences_and_each_mismatch_position() {
        unsafe {
            let mut a = [0x00u8, 0xFF, 0x80, b'a', 0];
            let mut b = a;
            let mut x = [0xABCDu32, a.as_mut_ptr() as u32, a.as_mut_ptr().add(5) as u32];
            let mut y = [0xDCBAu32, b.as_mut_ptr() as u32, b.as_mut_ptr().add(5) as u32];
            let xp = x.as_mut_ptr().cast();
            let yp = y.as_mut_ptr().cast();
            assert_eq!(fc4p_0063c020(xp, yp), 1);
            for value in &mut b {
                *value ^= 0x80;
                assert_eq!(fc4p_0063c020(xp, yp), 0);
                *value ^= 0x80;
            }
            put(yp, 8, word(yp, 8).wrapping_sub(1));
            assert_eq!(fc4p_0063c020(xp, yp), registers(0, yp.wrapping_add(4) as u32));
            put(xp, 4, 1);
            put(xp, 8, 1);
            put(yp, 4, 0);
            put(yp, 8, 0);
            assert_eq!(fc4p_0063c020(xp, yp), 1);
            assert_eq!(a, [0, 0xFF, 0x80, b'a', 0]);
        }
    }

    #[test]
    fn coordinate_tree_lower_bound_uses_signed_lexicographic_order_and_returns_header_at_end() {
        let mut header = [0u32; 6];
        let mut nodes = [[0u32; 6]; 5];
        let keys = [(i32::MIN, 8), (-1, -2), (-1, 0), (-1, i32::MAX), (i32::MAX, i32::MIN)];
        let pointers: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
        header[1] = pointers[2];
        nodes[2][2] = pointers[1];
        nodes[2][3] = pointers[3];
        nodes[1][2] = pointers[0];
        nodes[3][3] = pointers[4];
        for (node, key) in nodes.iter_mut().zip(keys) {
            node[4] = key.0 as u32;
            node[5] = key.1 as u32;
        }
        let mut tree = [header.as_mut_ptr() as u32];
        let p = tree.as_mut_ptr().cast();
        let before = nodes;
        for key in [
            (i32::MIN, i32::MIN),
            (i32::MIN, 8),
            (-2, 0),
            (-1, -2),
            (-1, -1),
            (-1, 0),
            (-1, 1),
            (0, 0),
            (i32::MAX, i32::MIN),
            (i32::MAX, i32::MAX),
        ] {
            let expected = keys.iter().position(|&k| k >= key).map_or(tree[0], |i| pointers[i]);
            unsafe {
                assert_eq!(fc4p_0063c070(p, [key.0, key.1].as_ptr()) as u32, expected);
            }
        }
        assert_eq!(nodes, before);
        unsafe {
            put(header.as_mut_ptr().cast(), 4, 0);
        }
        unsafe {
            assert_eq!(fc4p_0063c070(p, core::ptr::null()) as u32, tree[0]);
        }
    }

    #[test]
    fn typed_word_reads_after_the_type_call_and_preserves_payload_upper_bytes() {
        reset();
        let _type = fake(0x0063_DDF0, kind as *const () as usize);
        let mut object = Object::new(0x30);
        let p = object.ptr();
        let mut output = [0xA5A5_A5A5u32, 0x1234_5678, 0x5A5A_5A5A];
        for value in [0, 0x100, 0x101, 0xFFFF_FF00, 0xAABB_CC80] {
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.kind = value;
                s.mutate = Some((p as usize, 0x20, 0xFEDC_BA98));
            });
            let before = output;
            let output_pointer = (&mut output[1] as *mut u32) as u32;
            let regs = unsafe { run_registers(fc4p_0063e9d0 as *const (), Conv::Thiscall, p, &[output_pointer], 0, 0) };
            let result = regs[0];
            assert_eq!(regs[2], 0x5566_7788);
            assert_eq!(regs[1], if value == 0x100 { output_pointer } else { 0x1122_3344 });
            if value == 0x100 {
                assert_eq!(result, 0xFEDC_BA01);
                assert_eq!(output, [before[0], 0xFEDC_BA98, before[2]]);
            } else {
                assert_eq!(result, value & !0xFF);
                assert_eq!(output, before);
            }
            assert_eq!(calls(), [Call(0x0063_DDF0, p as usize, vec![])]);
        }
    }

    #[unsafe(naked)]
    unsafe extern "C" fn forwarded() {
        core::arch::naked_asm!("xor eax, [esp + 4]", "add eax, [esp + 8]", "xor edx, ecx", "ret 8");
    }

    #[test]
    fn forwarding_entries_keep_arguments_and_both_return_registers() {
        let entries = [
            (fc4p_006480d0 as *const (), 4, false, 0x0065_4120),
            (fc4p_006480e0 as *const (), 4, false, 0x0065_4130),
            (fc4p_006481d0 as *const (), 0x24C, true, 0x0065_4120),
            (fc4p_006481e0 as *const (), 0x24C, true, 0x0065_4130),
            (fc4p_00648200 as *const (), 0x1E4, true, 0x0080_1860),
            (fc4p_00648210 as *const (), 0x1E4, true, 0x0080_1890),
            (fc4p_00648220 as *const (), 0x1E4, true, 0x0080_18A0),
            (fc4p_00648240 as *const (), 0, true, 0x0065_4140),
            (fc4p_00648250 as *const (), 0, true, 0x0065_4500),
            (fc4p_00648260 as *const (), 0, true, 0x0065_4590),
            (fc4p_00648290 as *const (), 0x1E4, true, 0x0080_18E0),
            (fc4p_006482b0 as *const (), 0x1E4, true, 0x0080_1E10),
            (fc4p_006485d0 as *const (), 4, false, 0x0065_A860),
            (fc4p_006485e0 as *const (), 4, false, 0x0065_A870),
            (fc4p_00648600 as *const (), 8, false, 0x0065_A860),
            (fc4p_00648610 as *const (), 8, false, 0x0065_A870),
            (fc4p_006491d0 as *const (), -8i32 as u32, false, 0x005E_8DF0),
            (fc4p_0064dd70 as *const (), 4, false, 0x0064_91C0),
            (fc4p_0064dd80 as *const (), 4, false, 0x0064_91D0),
            (fc4p_0064dd90 as *const (), 8, false, 0x0064_91C0),
            (fc4p_0064dda0 as *const (), 8, false, 0x0064_91D0),
            (fc4p_0064ddb0 as *const (), 12, false, 0x0064_91C0),
            (fc4p_0064ddc0 as *const (), 12, false, 0x0064_91D0),
            (fc4p_0064ddd0 as *const (), 16, false, 0x0064_91C0),
            (fc4p_0064dde0 as *const (), 16, false, 0x0064_91D0),
            (fc4p_00652e50 as *const (), 4, false, 0x0065_2CF0),
            (fc4p_0068aff0 as *const (), -4i32 as u32, false, 0x006E_94C0),
            (fc4p_0068b000 as *const (), -4i32 as u32, false, 0x006E_A2F0),
            (fc4p_0068b1d0 as *const (), -4i32 as u32, false, 0x004F_7BA0),
            (fc4p_0068b1e0 as *const (), -4i32 as u32, false, 0x006E_8F90),
            (fc4p_0068d230 as *const (), 4, false, 0x004F_7BA0),
            (fc4p_0068d250 as *const (), 8, false, 0x004F_7BA0),
            (fc4p_006b8ab0 as *const (), 4, false, 0x006D_5140),
            (fc4p_006b8ac0 as *const (), 8, false, 0x006D_5140),
        ];
        for (entry, adjust, virtual_base, target) in entries {
            let _fake = fake(target, forwarded as *const () as usize);
            for displacement in [0u32, 4, 0x1234_5678, u32::MAX] {
                let mut object = [displacement, 0xAABB_CCDD];
                let p = unsafe { object.as_mut_ptr().add(1).cast::<u8>() };
                let expected_this =
                    (p as u32)
                        .wrapping_sub(adjust)
                        .wrapping_sub(if virtual_base { displacement } else { 0 });
                let result = unsafe {
                    run(
                        entry,
                        Conv::Thiscall,
                        p,
                        &[0xFEDC_BA98, 0x5432_1001],
                        0x1234_5678,
                        0xAABB_CCDD,
                    )
                };
                assert_eq!(
                    result,
                    (
                        (0x1234_5678u32 ^ 0xFEDC_BA98).wrapping_add(0x5432_1001),
                        0xAABB_CCDD ^ expected_this
                    )
                );
                assert_eq!(object, [displacement, 0xAABB_CCDD]);
            }
        }
    }

    #[test]
    fn constant_class_ids_match_the_network_and_developer_families() {
        for (entry, id) in [
            (fc4p_00648f40 as *const (), 0x49C0_5B9F),
            (fc4p_00653d90 as *const (), 0xCA16_374F),
            (fc4p_0068afd0 as *const (), 0xAA20_49A1),
            (fc4p_006b8b10 as *const (), 0xEA09_D4AE),
        ] {
            let result = unsafe { run(entry, Conv::Thiscall, core::ptr::null_mut(), &[], u32::MAX, 0xAABB_CCDD) };
            assert_eq!(result, (id, 0xAABB_CCDD));
        }
    }

    #[test]
    fn argument_entries_keep_upper_bits_and_pop_the_original_word_count() {
        let mut object = [0xA5u8; 32];
        let p = object.as_mut_ptr();
        unsafe {
            assert_eq!(
                run(
                    fc4p_00648f30 as *const (),
                    Conv::Thiscall,
                    p,
                    &[0xAABB_CCDD],
                    0x1122_3344,
                    0x5566_7788
                ),
                (0xAABB_CCDD, 0x5566_7788)
            );
            assert_eq!(
                run(
                    fc4p_00657730 as *const (),
                    Conv::Thiscall,
                    p,
                    &[1, 2, 3],
                    0x1122_3344,
                    0x5566_7788
                ),
                (0x1122_3344, 0x5566_7788)
            );
            for value in [0, 1, 0x80, 0xFF, 0xAABB_CC00, 0xAABB_CCDD] {
                let before = object;
                assert_eq!(
                    run(
                        fc4p_0067cb10 as *const (),
                        Conv::Thiscall,
                        p,
                        &[value],
                        0x1234_5678,
                        0x5566_7788
                    ),
                    (0x1234_5600 | (value & 0xFF), 0x5566_7788)
                );
                let mut expected = before;
                expected[0x10] = value as u8;
                assert_eq!(object, expected);
            }
        }
    }

    #[test]
    fn middle_pair_tests_only_the_low_byte_for_all_byte_values() {
        for high in [0, 0x1234_5600, 0xFFFF_FF00] {
            for low in 0..=255 {
                unsafe {
                    assert_eq!(
                        run_registers(
                            fc4p_00695260 as *const (),
                            Conv::Cdecl,
                            0xAABB_CCDD as *mut u8,
                            &[high | low],
                            0,
                            0x5566_7788
                        ),
                        [u32::from(low == 2 || low == 3), 0xAABB_CCDD, 0x5566_7788]
                    );
                }
            }
        }
    }

    #[test]
    fn reference_count_wraps_and_only_changes_its_word() {
        let mut object = [0xA5u8; 32];
        let p = object.as_mut_ptr();
        for count in [0, 1, i32::MAX as u32, 0x8000_0000, u32::MAX] {
            unsafe {
                put(p, 0x14, count);
            }
            let mut expected = object;
            expected[0x14..0x18].copy_from_slice(&count.wrapping_add(1).to_le_bytes());
            unsafe {
                assert_eq!(fc4p_00695330(p), count.wrapping_add(1));
            }
            assert_eq!(object, expected);
        }
    }

    #[test]
    fn consumption_and_borrowing_return_the_stored_bits_without_writes() {
        let mut object = [0xA5u8; 0x60];
        let p = object.as_mut_ptr();
        for value in [0, 1, u32::MAX, 0x8000_0000, 0xDEAD_BEEF] {
            unsafe {
                put(p, 0x54, value);
                put(p, 0x38, value);
                put(p, 0x3C, !value);
            }
            let before = object;
            unsafe {
                assert_eq!(
                    run(fc4p_006aaeb0 as *const (), Conv::Thiscall, p, &[], 0, 0x1234_5678),
                    (value, 0x1234_5678)
                );
                assert_eq!(
                    run(fc4p_006b9850 as *const (), Conv::Thiscall, p, &[], 0, 0),
                    (value, !value)
                );
            }
            assert_eq!(object, before);
        }
    }

    #[derive(Default)]
    struct Extra {
        owner: usize,
        allocation: usize,
        alternate: usize,
        snapshots: Vec<Vec<u8>>,
        pairs: [u32; 2],
        results: [u32; 2],
        service_calls: usize,
    }

    thread_local! { static EXTRA: RefCell<Extra> = RefCell::new(Extra::default()); }

    extern "C" fn free_buffer(p: *mut u8) -> u64 {
        log(0x0090_CF63, p, &[]);
        STATE.with(|s| {
            if let Some((owner, offset, value)) = s.borrow_mut().mutate.take() {
                unsafe {
                    put(owner as *mut u8, offset, value);
                }
            }
        });
        registers(0xFAFA_BCDE, (p as u32) ^ 0xDEAD_BEEF)
    }

    extern "thiscall" fn subtree(this: *mut u8, root: *mut u8) {
        log(0x0063_D700, this, &[root as u32]);
    }

    #[test]
    fn storage_destruction_clears_only_nonempty_tree_links_and_frees_buffers_in_order() {
        reset();
        let _free = fake(0x0090_CF63, free_buffer as *const () as usize);
        let _tree = fake(0x0063_D700, subtree as *const () as usize);
        let offsets = [0x15C, 0x8C, 0x80, 0x6C, 0x60, 0x50, 0x44, 0x20, 0x14];
        for (count, present) in [(0, false), (0, true), (3, true)] {
            for mask in [0u32, 0b101010101, 0b111111111] {
                let mut object = [0xA5u8; 0x180];
                let p = object.as_mut_ptr();
                let mut header = [0xABAB_ABABu32, 0x1234_5678, 0x8765_4321, 0xCAFE_BABE];
                let h = header.as_mut_ptr().cast::<u8>();
                unsafe {
                    put(p, 0x16C, if present { h as u32 } else { 0 });
                    put(p, 0x170, count);
                    for (i, &offset) in offsets.iter().enumerate() {
                        put(
                            p,
                            offset,
                            if mask & (1 << i) != 0 {
                                0x1000 + (i as u32) * 16
                            } else {
                                0
                            },
                        );
                    }
                }
                let mut expected_object = object;
                expected_object[0x158..0x15C].copy_from_slice(&0x00A8_0810u32.to_le_bytes());
                if count != 0 {
                    expected_object[0x170..0x174].fill(0);
                }
                let mut expected = vec![];
                if count != 0 {
                    expected.push(Call(0x0063_D700, p.wrapping_add(0x16C) as usize, vec![0x1234_5678]));
                }
                let mut edx = 0x1234_5678;
                if present {
                    expected.push(Call(0x0090_CF63, h as usize, vec![]));
                    edx = (h as u32) ^ 0xDEAD_BEEF;
                }
                let mut eax = 0;
                for (i, &offset) in offsets.iter().enumerate() {
                    let value = unsafe { word(p, offset) };
                    if i < 8 {
                        eax = value;
                    }
                    if value != 0 {
                        expected.push(Call(0x0090_CF63, value as usize, vec![]));
                        eax = 0xFAFA_BCDE;
                        edx = value ^ 0xDEAD_BEEF;
                    }
                }
                unsafe {
                    assert_eq!(
                        run(fc4p_00640230 as *const (), Conv::Thiscall, p, &[], 0, 0x1234_5678),
                        (eax, edx)
                    );
                }
                assert_eq!(object, expected_object);
                assert_eq!(
                    header,
                    if count == 0 {
                        [0xABAB_ABAB, 0x1234_5678, 0x8765_4321, 0xCAFE_BABE]
                    } else {
                        [0xABAB_ABAB, 0, h as u32, h as u32]
                    }
                );
                assert_eq!(calls(), expected);
            }
        }
        // A free callback can install a buffer into a field that has not been visited yet.
        let mut object = [0u32; 0x180 / 4];
        let p = object.as_mut_ptr().cast::<u8>();
        unsafe {
            put(p, 0x15C, 0x1020);
        }
        STATE.with(|s| s.borrow_mut().mutate = Some((p as usize, 0x20, 0x3040)));
        unsafe {
            assert_eq!(fc4p_00640230(p, 0), registers(0xFAFA_BCDE, 0x3040 ^ 0xDEAD_BEEF));
        }
        assert_eq!(
            calls(),
            [Call(0x0090_CF63, 0x1020, vec![]), Call(0x0090_CF63, 0x3040, vec![])]
        );
    }

    extern "thiscall" fn delegate_init(this: *mut u8) {
        log(0x0090_D957, this, &[]);
        EXTRA.with(|s| {
            let mut s = s.borrow_mut();
            s.snapshots.push(bytes(this.wrapping_sub(12), 0x134));
        });
        unsafe {
            put(this, 4, 0x0102_0304);
        }
    }

    extern "C" fn allocate_buffer(size: u32) -> *mut u8 {
        log(0x0090_CF54, core::ptr::null_mut(), &[size]);
        EXTRA.with(|s| {
            let mut s = s.borrow_mut();
            let owner = s.owner as *mut u8;
            s.snapshots.push(bytes(owner, 0x134));
            unsafe {
                put(owner, 0x14, 0x1122_3344);
                put(owner, 0x60, 0x5566_7788);
            }
            s.allocation as *mut u8
        })
    }

    #[test]
    fn constructor_preserves_gaps_and_exposes_the_expected_fields_to_each_callback() {
        reset();
        let _delegate = fake(0x0090_D957, delegate_init as *const () as usize);
        let _allocate = fake(0x0090_CF54, allocate_buffer as *const () as usize);
        let mut object = [0xA5u8; 0x144];
        let p = object.as_mut_ptr();
        let mut buffer = [0xCCu8; 8];
        let b = buffer.as_mut_ptr();
        EXTRA.with(|s| {
            *s.borrow_mut() = Extra {
                owner: p as usize,
                allocation: b as usize,
                ..Extra::default()
            }
        });
        unsafe {
            assert_eq!(fc4p_00645d50(p), p);
        }
        let snapshots = EXTRA.with(|s| core::mem::take(&mut s.borrow_mut().snapshots));
        let mut before_delegate = vec![0xA5u8; 0x134];
        for (offset, value) in [(0, 0x00AA_90C0u32), (4, 0x00A8_3524), (8, 0x00A8_1174)] {
            before_delegate[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        assert_eq!(snapshots[0], before_delegate);
        let mut before_allocate = before_delegate;
        for (offset, value) in [
            (0, 0x00AA_9140u32),
            (4, 0x00AA_9128),
            (8, 0x00AA_9118),
            (12, 0x00AA_9104),
            (16, 0x0102_0304),
        ] {
            before_allocate[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for offset in [0x14, 0x18, 0x2C, 0x30, 0x44, 0x48, 0x4C, 0x54, 0x58, 0x5C] {
            before_allocate[offset..offset + 4].fill(0);
        }
        for offset in [0x24, 0x25, 0x26, 0x27, 0x29, 0x2A, 0x2B] {
            before_allocate[offset] = 0;
        }
        assert_eq!(snapshots[1], before_allocate);
        let mut expected = before_allocate;
        expected.extend_from_slice(&[0xA5; 16]);
        expected[0x14..0x18].copy_from_slice(&0x1122_3344u32.to_le_bytes());
        for (offset, value) in [
            (0x50, 0x00A8_0810u32),
            (0x54, b as u32),
            (0x58, b as u32),
            (0x5C, b.wrapping_add(8) as u32),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        for offset in [
            0x60, 0x74, 0x78, 0x7C, 0xAC, 0xB0, 0xB4, 0xD8, 0xDC, 0xE0, 0xE4, 0xE8, 0xEC, 0xF0, 0xF4, 0xF8, 0x124,
            0x128, 0x12C, 0x130,
        ] {
            expected[offset..offset + 4].fill(0);
        }
        assert_eq!(object.as_slice(), expected);
        assert_eq!(buffer, [0, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC, 0xCC]);
        assert_eq!(
            calls(),
            [
                Call(0x0090_D957, p.wrapping_add(12) as usize, vec![]),
                Call(0x0090_CF54, 0, vec![8])
            ]
        );
    }

    #[test]
    fn backward_copy_keeps_word_order_inside_overlapping_records_and_signed_empty_counts() {
        unsafe {
            let mut data = [1u32, 2, 3, 4, 5, 6, 7, 8, 9, 10];
            let p = data.as_mut_ptr().cast::<u8>();
            let result = fc4p_0066b340(p, p.add(24), p.add(28));
            assert_eq!(result, registers(p.add(4) as u32, u32::MAX - 3));
            assert_eq!(data, [1, 1, 1, 3, 3, 5, 5, 8, 9, 10]);
            let before = data;
            for last in [p, p.add(7), p.wrapping_sub(8)] {
                assert_eq!(
                    fc4p_0066b340(p, last, p.add(32)),
                    registers(p.add(32) as u32, last as u32)
                );
                assert_eq!(data, before);
            }
            let mut source = [11u32, 12, 13, 14];
            let s = source.as_mut_ptr().cast::<u8>();
            assert_eq!(
                fc4p_0066b340(s, s.add(16), p.add(20)),
                registers(p.add(4) as u32, (s.add(16) as u32).wrapping_sub(p.add(20) as u32))
            );
            assert_eq!(data, [1, 11, 12, 13, 14, 5, 5, 8, 9, 10]);
        }
    }

    extern "thiscall" fn set_key(this: *mut u8, key: *mut u8) -> u64 {
        log(0x0097_5402, this, &[key as u32]);
        registers(0x1122_3344, 0x5566_7788)
    }

    extern "thiscall" fn get_key(this: *mut u8, output: *mut u32) -> *mut u8 {
        log(0x0097_53EA, this, &[]);
        unsafe {
            output.write(0x1122_3344);
            output.add(1).write(0xAABB_CCDD);
            output.add(2).write(0xDEAD_BEEF);
        }
        EXTRA.with(|s| {
            let p = s.borrow().alternate;
            if p == 0 {
                output.cast()
            } else {
                p as *mut u8
            }
        })
    }

    extern "C" fn lower_pair(first: *mut u8, last: *mut u8, key: *const u32, value: u32, cookie: u32) -> u64 {
        log(
            0x005F_E940,
            first,
            &[
                last as u32,
                unsafe { key.read() },
                unsafe { key.add(1).read() },
                value,
                cookie,
            ],
        );
        STATE.with(|s| {
            if let Some((p, offset, value)) = s.borrow().mutate {
                unsafe {
                    put(p as *mut u8, offset, value);
                }
            }
        });
        EXTRA.with(|s| registers(s.borrow().alternate as u32, 0x8765_4321))
    }

    #[test]
    fn embedded_resource_keys_keep_all_result_bytes_and_copy_aliased_words_in_order() {
        reset();
        let _set = fake(0x0097_5402, set_key as *const () as usize);
        let _get = fake(0x0097_53EA, get_key as *const () as usize);
        let mut object = [0xA5u8; 32];
        let p = object.as_mut_ptr();
        let mut key = [0u32; 5];
        let k = key.as_mut_ptr().cast::<u8>();
        unsafe {
            assert_eq!(fc4p_0067c220(p, k), registers(0x1122_3301, 0x5566_7788));
        }
        assert_eq!(calls(), [Call(0x0097_5402, p.wrapping_add(4) as usize, vec![k as u32])]);
        EXTRA.with(|s| *s.borrow_mut() = Extra::default());
        unsafe {
            assert_eq!(fc4p_0067cd20(p, k.add(4)), registers(0xDEAD_BE01, 0xAABB_CCDD));
        }
        assert_eq!(key, [0, 0x1122_3344, 0xAABB_CCDD, 0xDEAD_BEEF, 0]);
        assert_eq!(calls(), [Call(0x0097_53EA, p.wrapping_add(4) as usize, vec![])]);
        key = [0x1234_5678, 0xAABB_CCDD, 0xDEAD_BEEF, 0, 0xA5A5_A5A5];
        EXTRA.with(|s| s.borrow_mut().alternate = k as usize);
        unsafe {
            assert_eq!(fc4p_0067cd20(p, k.add(4)), registers(0x1234_5601, 0x1234_5678));
        }
        assert_eq!(key, [0x1234_5678, 0x1234_5678, 0x1234_5678, 0x1234_5678, 0xA5A5_A5A5]);
        assert_eq!(object, [0xA5; 32]);
    }

    #[test]
    fn pair_lookup_delegates_to_lower_bound_and_keeps_the_original_cached_end() {
        reset();
        let _lower = fake(0x005F_E940, lower_pair as *const () as usize);
        let mut object = [0u32; 8];
        let p = object.as_mut_ptr().cast::<u8>();
        let mut data = [3u32, 0x1122_3344, 9, 0x5566_7788];
        let a = data.as_mut_ptr().cast::<u8>();
        let b = a.wrapping_add(16);
        for (key, candidate, found, value) in [
            (3, a, true, 0x1122_3344),
            (9, a.wrapping_add(8), true, 0x5566_7788),
            (4, a.wrapping_add(8), false, 0),
            (u32::MAX, b, false, 0),
        ] {
            for getter in [false, true] {
                unsafe {
                    put(p, 0x18, a as u32);
                    put(p, 0x1C, b as u32);
                }
                EXTRA.with(|s| s.borrow_mut().alternate = candidate as usize);
                STATE.with(|s| s.borrow_mut().mutate = Some((p as usize, 0x1C, candidate as u32)));
                let result = unsafe {
                    if getter {
                        fc4p_0067c670(p, key)
                    } else {
                        fc4p_0067c620(p, key)
                    }
                };
                let eax = if getter {
                    value
                } else {
                    (candidate as u32 & !0xFF) | u32::from(found)
                };
                assert_eq!(result, registers(eax, 0x8765_4321));
                assert_eq!(calls(), [Call(0x005F_E940, a as usize, vec![b as u32, key, 0, key, 0])]);
            }
        }
        assert_eq!(data, [3, 0x1122_3344, 9, 0x5566_7788]);
    }

    extern "C" fn allocate_collection(size: u32) -> *mut u8 {
        log(0x005E_55E0, core::ptr::null_mut(), &[size]);
        EXTRA.with(|s| s.borrow().allocation as *mut u8)
    }

    extern "thiscall" fn collection_new(this: *mut u8) -> *mut u8 {
        log(0x0090_DC0F, this, &[]);
        EXTRA.with(|s| s.borrow().alternate as *mut u8)
    }

    extern "thiscall" fn collection_retain(this: *mut u8) -> u64 {
        log(4, this, &[]);
        EXTRA.with(|s| {
            let s = s.borrow();
            unsafe {
                put(s.owner as *mut u8, 0x18, s.pairs[0]);
                put(s.owner as *mut u8, 0x1C, s.pairs[1]);
            }
        });
        registers(0x1234_5678, 0xAABB_CCDD)
    }

    extern "thiscall" fn collection_append(this: *mut u8, value: u32) -> u64 {
        log(0x1C, this, &[value]);
        registers(0x5566_7700 | value, 0x8877_6655)
    }

    #[test]
    fn resource_collection_handles_null_output_and_reads_pairs_after_retaining_the_constructed_object() {
        reset();
        let _allocate = fake(0x005E_55E0, allocate_collection as *const () as usize);
        let _new = fake(0x0090_DC0F, collection_new as *const () as usize);
        let mut vt = [0usize; 8];
        vt[1] = collection_retain as *const () as usize;
        vt[7] = collection_append as *const () as usize;
        let mut collection = [vt.as_ptr() as usize; 3];
        let c = collection.as_mut_ptr().cast::<u8>();
        let mut object = [0u32; 8];
        let p = object.as_mut_ptr().cast::<u8>();
        let mut pairs = [1u32, 3, 2, 5, 3, 7];
        let a = pairs.as_mut_ptr() as u32;
        unsafe {
            assert_eq!(
                run(
                    fc4p_0067c4b0 as *const (),
                    Conv::Thiscall,
                    p,
                    &[0],
                    0x1122_3344,
                    0x5566_7788
                ),
                (0x1122_3300, 0x5566_7788)
            );
        }
        assert!(calls().is_empty());
        for count in 0..=3 {
            let mut output = core::ptr::null_mut::<u8>();
            EXTRA.with(|s| {
                *s.borrow_mut() = Extra {
                    owner: p as usize,
                    allocation: 0x1020,
                    alternate: c as usize,
                    pairs: [a, a + count * 8],
                    ..Extra::default()
                }
            });
            let result = unsafe {
                run(
                    fc4p_0067c4b0 as *const (),
                    Conv::Thiscall,
                    p,
                    &[(&mut output as *mut *mut u8) as u32],
                    0,
                    0,
                )
            };
            assert_eq!(output, c);
            let mut expected = vec![
                Call(0x005E_55E0, 0, vec![12]),
                Call(0x0090_DC0F, 0x1020, vec![]),
                Call(4, c as usize, vec![]),
            ];
            for value in [3u32, 5, 7].into_iter().take(count as usize) {
                expected.push(Call(0x1C, c as usize, vec![value]));
            }
            assert_eq!(calls(), expected);
            assert_eq!(
                result,
                if count == 0 {
                    (0x1234_5601, 0xAABB_CCDD)
                } else {
                    (0x5566_7701, 0x8877_6655)
                }
            );
        }
    }

    extern "thiscall" fn service(this: *mut u8, index: u32) -> *mut u8 {
        log(0x50, this, &[index]);
        EXTRA.with(|s| s.borrow().alternate as *mut u8)
    }

    extern "thiscall" fn service_method(this: *mut u8) -> u64 {
        log(0x2C, this, &[]);
        registers(0xDEAD_BEEF, 0xFEDC_BA98)
    }

    extern "thiscall" fn scaled_initialize(this: *mut u8, first: u32, second: u32) -> u64 {
        log(0x006F_6A70, this, &[first, second, unsafe { word(this, 0x98) }]);
        registers(0x1234_5678, 0x90AB_CDEF)
    }

    #[test]
    fn service_forwarder_and_unit_scale_initializer_keep_original_targets_arguments_and_results() {
        reset();
        let mut svt = [0usize; 0x54 / 4];
        svt[0x50 / 4] = service as *const () as usize;
        let mut target_vt = [0usize; 0x30 / 4];
        target_vt[0x2C / 4] = service_method as *const () as usize;
        let mut s = [svt.as_ptr() as usize];
        let sp = s.as_mut_ptr().cast::<u8>();
        let mut target = [target_vt.as_ptr() as usize];
        let t = target.as_mut_ptr().cast::<u8>();
        let mut object = [0xA5u8; 0xB0];
        let p = object.as_mut_ptr();
        unsafe {
            put(p, 0x30, sp as u32);
        }
        EXTRA.with(|s| s.borrow_mut().alternate = t as usize);
        let mut expected = object;
        unsafe {
            assert_eq!(fc4p_0068b270(p), registers(0xDEAD_BEEF, 0xFEDC_BA98));
        }
        assert_eq!(
            calls(),
            [Call(0x50, sp as usize, vec![1]), Call(0x2C, t as usize, vec![])]
        );
        assert_eq!(object, expected);
        let _initialize = fake(0x006F_6A70, scaled_initialize as *const () as usize);
        unsafe {
            assert_eq!(
                run(
                    fc4p_006acf70 as *const (),
                    Conv::Thiscall,
                    p,
                    &[0x1122_3344, 0x5566_7788],
                    0xAABB_CCDD,
                    0xEEFF_0011
                ),
                (0x1234_5678, 0x90AB_CDEF)
            );
        }
        expected[0x98..0x9C].copy_from_slice(&0x3F80_0000u32.to_le_bytes());
        assert_eq!(object, expected);
        assert_eq!(
            calls(),
            [Call(
                0x006F_6A70,
                p as usize,
                vec![0x1122_3344, 0x5566_7788, 0x3F80_0000]
            )]
        );
    }

    extern "thiscall" fn write_version(this: *mut u8, value: u32) -> u32 {
        log(0x1C, this, &[value]);
        0
    }
    extern "C" fn write_range(stream: *mut u8, range: *mut u8) {
        log(0x005D_AFF0, stream, &[range as u32]);
        unsafe {
            put(range, 8, 0xCAFE_BABE);
        }
    }
    extern "thiscall" fn write_word(this: *mut u8, value: u32) -> u32 {
        log(0x24, this, &[value]);
        0
    }
    extern "thiscall" fn stream_error(this: *mut u8) -> u64 {
        log(0x48, this, &[]);
        registers(EXTRA.with(|s| s.borrow().results[0]), 0x8765_4321)
    }

    #[test]
    fn range_writer_reloads_the_trailing_word_after_serialization_and_tests_only_the_error_byte() {
        reset();
        let _range = fake(0x005D_AFF0, write_range as *const () as usize);
        let mut vt = [0usize; 0x4C / 4];
        vt[0x1C / 4] = write_version as *const () as usize;
        vt[0x24 / 4] = write_word as *const () as usize;
        vt[0x48 / 4] = stream_error as *const () as usize;
        let mut stream = [vt.as_ptr() as usize];
        let st = stream.as_mut_ptr().cast::<u8>();
        let mut object = [0xA5u8; 0x30];
        let p = object.as_mut_ptr();
        for error in [0, 0x1234_5600, 1, 0x80, u32::MAX] {
            EXTRA.with(|s| s.borrow_mut().results[0] = error);
            let result = unsafe { fc4p_006ac930(p, st) };
            assert_eq!(result, registers(u32::from(error as u8 == 0), 0x8765_4321));
            assert_eq!(
                calls(),
                [
                    Call(0x1C, st as usize, vec![1]),
                    Call(0x005D_AFF0, st as usize, vec![p.wrapping_add(0x14) as u32]),
                    Call(0x24, st as usize, vec![0xCAFE_BABE]),
                    Call(0x48, st as usize, vec![])
                ]
            );
        }
        let mut expected = [0xA5u8; 0x30];
        expected[0x1C..0x20].copy_from_slice(&0xCAFE_BABEu32.to_le_bytes());
        assert_eq!(object, expected);
    }

    extern "thiscall" fn occupant_service(this: *mut u8) -> *mut u8 {
        log(0x144, this, &[]);
        EXTRA.with(|s| {
            let mut s = s.borrow_mut();
            let service = s.pairs[s.service_calls];
            s.service_calls += 1;
            service as *mut u8
        })
    }
    extern "C" fn first_occupant(x: u32, z: u32, output: *mut *mut u8, service: *mut u8) -> u64 {
        log(0x0065_84D0, service, &[x, z, unsafe { output.read() } as u32]);
        EXTRA.with(|s| {
            let s = s.borrow();
            unsafe {
                output.write(s.alternate as *mut u8);
            }
            registers(s.results[0], 0x1234_5678)
        })
    }
    extern "C" fn second_occupant(x: u32, z: u32, service: *mut u8, output: *mut *mut u8) -> u64 {
        log(0x0065_85E0, service, &[x, z, unsafe { output.read() } as u32]);
        EXTRA.with(|s| {
            let s = s.borrow();
            unsafe {
                output.write(s.alternate as *mut u8);
            }
            registers(s.results[1], 0x90AB_CDEF)
        })
    }

    #[test]
    fn occupant_search_uses_each_original_search_and_releases_only_the_first_successful_result() {
        reset();
        let _first = fake(0x0065_84D0, first_occupant as *const () as usize);
        let _second = fake(0x0065_85E0, second_occupant as *const () as usize);
        let mut vt = [0usize; 0x148 / 4];
        vt[0x144 / 4] = occupant_service as *const () as usize;
        let mut object = [vt.as_ptr() as usize];
        let p = object.as_mut_ptr().cast::<u8>();
        let release_vt = [0usize, 0, release as *const () as usize];
        let mut found = [release_vt.as_ptr() as usize];
        let f = found.as_mut_ptr().cast::<u8>();
        for results in [[0xABCD_0080, 1], [0xABCD_0000, 0xDCBA_0080], [0xABCD_0000, 0xDCBA_0000]] {
            EXTRA.with(|s| {
                *s.borrow_mut() = Extra {
                    alternate: f as usize,
                    pairs: [0x1020, 0x3040],
                    results,
                    ..Extra::default()
                }
            });
            let result = unsafe { fc4p_00659470(p, u32::MAX, 0x8000_0000) };
            let mut expected = vec![
                Call(0x144, p as usize, vec![]),
                Call(0x0065_84D0, 0x1020, vec![u32::MAX, 0x8000_0000, 0]),
            ];
            if results[0] as u8 == 0 {
                expected.push(Call(0x144, p as usize, vec![]));
                expected.push(Call(0x0065_85E0, 0x3040, vec![u32::MAX, 0x8000_0000, 0]));
            }
            if results[0] as u8 != 0 || results[1] as u8 != 0 {
                expected.push(Call(8, f as usize, vec![]));
                assert_eq!(result, registers(0xA1B2_C301, 0x0077_8899));
            } else {
                assert_eq!(result, registers(0xDCBA_0000, 0x90AB_CDEF));
            }
            assert_eq!(calls(), expected);
        }
    }
}
