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

// The boundary saves the incoming register words so Rust can retain partial results. All decisions,
// memory accesses and game calls live in the Rust body; the boundary only restores the result words.
macro_rules! register_entry {
    ($address:literal, $export:ident, $body:ident, $ret:literal, $name:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov edx, esp", "call {body}",
                    "pop eax", "pop ecx", "pop edx", $ret, body = sym $body,
                );
            }
        }
    };
}

layout! {
    /// Network-tool segment and extra-cell ranges.
    struct SegmentCheck {
        0x44 => terrain / set_terrain: *mut u8,
        0x54 => segments / set_segments: *mut u8,
        0x60 => coordinates / set_coordinates: *mut u8,
        0xF0 => extras / set_extras: *mut u8,
        0xF4 => extras_end / set_extras_end: *mut u8,
    }
}
layout! {
    /// Three-word storage descriptor for eight-byte elements.
    struct PairVector {
        0x00 => first / set_first: *mut u8,
        0x04 => last / set_last: *mut u8,
        0x08 => limit / set_limit: *mut u8,
    }
}
layout! {
    /// Parser byte cursor, byte limit and optional completion service.
    struct ParserStorage {
        0x0C => cursor / set_cursor: *mut u8,
        0x10 => limit / set_limit: *mut u8,
        0x30 => service / set_service: *mut u8,
    }
}
layout! {
    /// A measured sample and its shared adaptive batch state.
    struct BatchSample {
        0x08 => sample / set_sample: u32,
        0x14 => state / set_state: *mut u8,
    }
}
layout! {
    /// Batch size and signed, wrapping streak counter.
    struct BatchState {
        0x00 => size / set_size: u32,
        0x04 => counter / set_counter: i32,
    }
}
layout! {
    /// A retained-object list link and payload.
    struct RetainedNode {
        0x00 => next / set_next: *mut u8,
        0x08 => object / set_object: *mut u8,
    }
}

unsafe fn save_result(frame: *mut u32, result: u64) {
    unsafe {
        frame.write_unaligned(result as u32);
        frame.add(2).write_unaligned((result >> 32) as u32);
    }
}

/// Supplies the registers a callee may leave unchanged, as well as its stack words.
unsafe fn seeded_call(
    conv: crate::abi::Conv,
    target: usize,
    this: u32,
    args: &[u32],
    eax: u32,
    edx: u32,
) -> u64 {
    let mut call = crate::abi::RawCall::new(conv, target, this, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        crate::abi::call_raw(&mut call);
    }
    registers(call.eax, call.edx)
}

unsafe fn seeded_virtual(this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> u64 {
    unsafe {
        seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::vslot(this, slot),
            this as u32,
            args,
            eax,
            edx,
        )
    }
}

game_fn! { fn find_byte(first: *mut u8, byte: u32, length: u32) -> u64 = "C" 0x009F_0B40; }
game_fn! { fn move_pairs_backward(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 = "C" 0x0066_B340; }
game_fn! { #[allow(dead_code)] fn grow_pairs(this: *mut u8, at: *mut u8, value: *const u32, cookie: *mut u32, count: u32, mode: u32) -> u64 = "thiscall" 0x007C_61A0; }
game_fn! { fn copy_pairs(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 = "C" 0x0077_AC20; }
game_fn! { fn destroy_pairs(this: *mut u8) -> u64 = "thiscall" 0x005F_F0C0; }
game_fn! { fn clear_parser_values(this: *mut u8) -> u64 = "thiscall" 0x005F_F160; }
game_fn! { fn copy_pair_vector(this: *mut u8, source: *mut u8) -> *mut u8 = "thiscall" 0x005F_EA70; }
game_fn! { #[allow(dead_code)] fn next_nonempty_line(this: *mut u8, first: *mut *mut u8, last: *mut *mut u8) -> u64 = "thiscall" 0x0067_C240; }
game_fn! { fn assign_string_range(this: *mut u8, first: *mut u8, last: *mut u8) -> u64 = "thiscall" 0x0044_D3A0; }
game_fn! { #[allow(dead_code)] fn trim_string(this: *mut u8) -> u64 = "thiscall" 0x0090_E8A6; }
game_fn! { fn erase_list_node(this: *mut u8, output: *mut u32, node: *mut u8) -> u64 = "thiscall" 0x0041_3F70; }
game_fn! { #[allow(dead_code)] fn insert_integer_pair(this: *mut u8, output: *mut *mut u8, hint: *mut u8, value: *const u32) -> u64 = "thiscall" 0x0069_7530; }

register_entry!(
    0x0063_8990,
    fc4p_00638990,
    check_network_segments,
    "ret 4",
    "CheckNetworkSegmentsAndExtraCells"
);
unsafe extern "fastcall" fn check_network_segments(this: *mut u8, frame: *mut u32) {
    unsafe {
        let result = check_segments_and_extra_cells(
            this,
            frame.add(4).read_unaligned() as *mut i32,
            frame.add(2).read_unaligned(),
        );
        save_result(frame, result);
    }
}

/// Checks ordinary network cells and extra cells, recording the first rejecting segment.
/// The extra cells reject land that is absent or already active.
unsafe fn check_segments_and_extra_cells(this: *mut u8, count: *mut i32, entry_edx: u32) -> u64 {
    unsafe {
        let view = SegmentCheck::from_ptr(this);
        let gate = seeded_virtual(this, 0xFC, &[], word(this, 0), entry_edx);
        if gate as u8 == 0 {
            return (gate & !0xFF) | 1;
        }
        let mut edx = count.read_unaligned() as u32;
        let mut segment = 0i32;
        let mut offset = 0usize;
        while segment < count.read_unaligned() {
            let row = view.segments().wrapping_add(offset);
            if word(row, 8) == 0 {
                let mut index = word(row, 0);
                if index <= word(row, 4) {
                    loop {
                        let pair = view
                            .coordinates()
                            .wrapping_add(index.wrapping_mul(8) as usize);
                        let (x, z) = (word(pair, 0), word(pair, 4));
                        let terrain = view.terrain();
                        let land =
                            seeded_virtual(terrain, 0x88, &[x, z], pair as u32, word(terrain, 0));
                        if land as u8 != 0 {
                            let found = seeded_call(
                                crate::abi::Conv::Thiscall,
                                crate::abi::target(0x0063_3220),
                                this as u32,
                                &[z.wrapping_shl(16).wrapping_add(x)],
                                land as u32,
                                (land >> 32) as u32,
                            );
                            let cell = found as u32 as *mut u8;
                            if cell.is_null() || *cell.add(0x56) == 0 {
                                count.write_unaligned(segment);
                                return registers((segment as u32) & !0xFF, (found >> 32) as u32);
                            }
                        }
                        let end = word(view.segments(), offset + 4);
                        index = index.wrapping_add(1);
                        if index > end {
                            break;
                        }
                    }
                }
            }
            edx = count as u32;
            segment = segment.wrapping_add(1);
            offset = offset.wrapping_add(12);
        }
        let end = view.extras_end();
        let mut extra = view.extras();
        while extra != end {
            let (x, z) = (word(extra, 0), word(extra, 4));
            let terrain = view.terrain();
            let land = seeded_virtual(terrain, 0x88, &[x, z], end as u32, word(terrain, 0));
            if land as u8 == 0 {
                count.write_unaligned(0);
                return registers((count as u32) & !0xFF, (land >> 32) as u32);
            }
            let found = seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x0063_3220),
                this as u32,
                &[z.wrapping_shl(16).wrapping_add(x)],
                land as u32,
                (land >> 32) as u32,
            );
            edx = (found >> 32) as u32;
            let cell = found as u32 as *mut u8;
            if !cell.is_null() && *cell.add(0x56) != 0 {
                count.write_unaligned(0);
                return registers((count as u32) & !0xFF, edx);
            }
            extra = extra.wrapping_add(20);
        }
        registers(((end as u32) & !0xFF) | 1, edx)
    }
}

register_entry!(
    0x0065_7740,
    fc4p_00657740,
    fetch_optional_word,
    "ret 16",
    "FetchOptionalWord"
);
unsafe extern "fastcall" fn fetch_optional_word(this: *mut u8, frame: *mut u32) {
    unsafe {
        let out = frame.add(5).read_unaligned() as *mut u32;
        if !out.is_null() {
            let a = frame.add(4).read_unaligned();
            let result = seeded_virtual(
                this,
                0x148,
                &[
                    a,
                    frame.add(5) as u32,
                    frame.add(6).read_unaligned(),
                    frame.add(7).read_unaligned(),
                ],
                word(this, 0),
                a,
            );
            let value = frame.add(5).read_unaligned();
            out.write_unaligned(value);
            save_result(frame, registers(value, (result >> 32) as u32));
        }
    }
}

register_entry!(
    0x0066_5DB0,
    fc4p_00665db0,
    insert_pair,
    "ret 8",
    "InsertPair"
);
unsafe extern "fastcall" fn insert_pair(this: *mut u8, frame: *mut u32) {
    unsafe {
        let first = word(this, 0);
        let end = pointer(this, 4);
        let at = frame.add(4).read_unaligned() as *mut u8;
        let source = frame.add(5).read_unaligned() as *const u32;
        let index = (at as u32).wrapping_sub(first) as i32 >> 3;
        let edx;
        if end == pointer(this, 8) {
            let result = seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x007C_61A0),
                this as u32,
                &[at as u32, source as u32, frame.add(5) as u32, 1, 0],
                end as u32,
                source as u32,
            );
            edx = (result >> 32) as u32;
        } else if at == end {
            edx = if end.is_null() {
                frame.add(2).read_unaligned()
            } else {
                let value = source.read_unaligned();
                put(end, 0, value);
                put(end, 4, source.add(1).read_unaligned());
                value
            };
            put(this, 4, word(this, 4).wrapping_add(8));
        } else {
            if !end.is_null() {
                put(end, 0, word(end.wrapping_sub(8), 0));
                put(end, 4, word(end.wrapping_sub(8), 4));
            }
            let new_end = pointer(this, 4).wrapping_add(8);
            put(this, 4, new_end as u32);
            let second = source.add(1).read_unaligned();
            let first = source.read_unaligned();
            let result = move_pairs_backward(
                at,
                new_end.wrapping_sub(16),
                new_end.wrapping_sub(8),
                frame.add(5),
            );
            put(at, 0, first);
            put(at, 4, second);
            edx = (result >> 32) as u32;
        }
        save_result(
            frame,
            registers(
                word(this, 0).wrapping_add((index as u32).wrapping_mul(8)),
                edx,
            ),
        );
    }
}

register_entry!(
    0x0067_C240,
    fc4p_0067c240,
    scan_nonempty_line,
    "ret 8",
    "ScanNonemptyLine"
);
unsafe extern "fastcall" fn scan_nonempty_line(this: *mut u8, frame: *mut u32) {
    unsafe {
        let out_first = frame.add(4).read_unaligned() as *mut u32;
        let out_last = frame.add(5).read_unaligned() as *mut u32;
        let mut eax = word(this, 0x0C);
        let mut edx = frame.add(2).read_unaligned();
        while eax != word(this, 0x10) {
            let start = pointer(this, 0x0C);
            let length = word(this, 0x10).wrapping_sub(start as u32);
            let result = find_byte(start, 10, length);
            eax = result as u32;
            edx = word(this, 0x0C);
            out_first.write_unaligned(edx);
            if eax == 0 {
                eax = word(this, 0x10);
                put(this, 0x0C, eax);
            } else {
                put(this, 0x0C, eax.wrapping_add(1));
            }
            out_last.write_unaligned(eax);
            if eax > out_first.read_unaligned() {
                let last_byte = *((eax as *const u8).wrapping_sub(1));
                eax = eax.wrapping_sub(1);
                if last_byte == 13 {
                    out_last.write_unaligned(eax);
                }
            }
            edx = out_last.read_unaligned();
            if edx > out_first.read_unaligned() {
                save_result(frame, registers((eax & !0xFF) | 1, edx));
                return;
            }
            eax = word(this, 0x0C);
        }
        save_result(frame, registers(eax & !0xFF, edx));
    }
}

register_entry!(
    0x0067_CB20,
    fc4p_0067cb20,
    reserve_pairs,
    "ret 4",
    "ReservePairs"
);
unsafe extern "fastcall" fn reserve_pairs(this: *mut u8, frame: *mut u32) {
    unsafe {
        let vector = PairVector::from_ptr(this);
        let first = vector.first();
        let capacity = (vector.limit() as u32).wrapping_sub(first as u32) as i32 >> 3;
        let mut requested = frame.add(4).read_unaligned();
        if capacity as u32 >= requested {
            frame.write_unaligned(capacity as u32);
            return;
        }
        let end = vector.last();
        let len = (end as u32).wrapping_sub(first as u32) as i32 >> 3;
        let allocation = if requested == 0 {
            core::ptr::null_mut()
        } else {
            allocate_storage(requested.wrapping_mul(8))
        };
        if !first.is_null() {
            copy_pairs(first, end, allocation, frame.add(4));
            destroy_pairs(this);
            requested = frame.add(4).read_unaligned();
        }
        let new_end = (allocation as u32).wrapping_add((len as u32).wrapping_mul(8));
        let limit = (allocation as u32).wrapping_add(requested.wrapping_mul(8));
        vector.set_first(allocation);
        vector.set_last(new_end as *mut u8);
        vector.set_limit(limit as *mut u8);
        save_result(frame, registers(limit, new_end));
    }
}

register_entry!(
    0x0067_CD50,
    fc4p_0067cd50,
    compact_parser_storage,
    "ret",
    "CompactParserStorage"
);
unsafe extern "fastcall" fn compact_parser_storage(this: *mut u8, frame: *mut u32) {
    unsafe {
        clear_parser_values(this.wrapping_add(0x1C));
        let vector = this.wrapping_add(0x34);
        let first = word(vector, 0);
        let size = word(vector, 4).wrapping_sub(first);
        let capacity = word(vector, 8).wrapping_sub(first);
        let mut edx = first;
        if (size ^ capacity) & !7 != 0 {
            let temporary = frame.cast::<u8>();
            let copied = copy_pair_vector(temporary, vector);
            for offset in [0, 4, 8] {
                let value = word(vector, offset);
                let old = word(copied, offset);
                put(copied, offset, value);
                put(vector, offset, old);
            }
            let original_limit = word(copied, 8);
            edx = (seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x005F_F0C0),
                temporary as u32,
                &[],
                copied as u32,
                original_limit,
            ) >> 32) as u32;
        }
        let service = ParserStorage::from_ptr(this).service();
        let result = if service.is_null() {
            registers(0, edx)
        } else {
            seeded_virtual(service, 0x34, &[], word(service, 0), edx)
        };
        save_result(frame, result);
    }
}

register_entry!(
    0x0067_E0B0,
    fc4p_0067e0b0,
    read_key_value_line,
    "ret 8",
    "ReadKeyValueLine"
);
unsafe extern "fastcall" fn read_key_value_line(this: *mut u8, frame: *mut u32) {
    unsafe {
        let key = frame.add(4).read_unaligned() as *mut u8;
        let value = frame.add(5).read_unaligned() as *mut u8;
        // These two slots occupy the original line-range locals, including a returned stack address.
        let first_out = frame.add(2).cast::<*mut u8>();
        let last_out = frame.add(1).cast::<*mut u8>();
        let line = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0067_C240),
            this as u32,
            &[first_out as u32, last_out as u32],
            last_out as u32,
            first_out as u32,
        );
        if line as u8 == 0 {
            save_result(frame, line);
            return;
        }
        let first = first_out.read_unaligned();
        let last = last_out.read_unaligned();
        let found = find_byte(first, 61, (last as u32).wrapping_sub(first as u32));
        let equals = found as u32 as *mut u8;
        if equals.is_null() {
            save_result(frame, found & !0xFF);
            return;
        }
        let assigned = assign_string_range(key.wrapping_add(4), first, equals);
        seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0090_E8A6),
            key as u32,
            &[],
            assigned as u32,
            (assigned >> 32) as u32,
        );
        let assigned = assign_string_range(value.wrapping_add(4), equals.wrapping_add(1), last);
        let trimmed = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0090_E8A6),
            value as u32,
            &[],
            assigned as u32,
            (assigned >> 32) as u32,
        );
        save_result(frame, (trimmed & !0xFF) | 1);
    }
}

port! {
    /// Adjusts a batch size after sixteen slow samples or four fast samples.
    /// Samples are measured in units of 1024; the counter wraps as a signed word.
    #[port(address = 0x0068_B960, owner = "shared", name = "AdjustAdaptiveBatchSize")]
    pub unsafe extern "thiscall" fn fc4p_0068b960(this: *mut u8) -> u64 {
        unsafe {
            let view = BatchSample::from_ptr(this);
            let sample = view.sample() >> 10;
            if sample == 0 || (5..=20).contains(&sample) { return registers(sample, 0); }
            let state = BatchState::from_ptr(view.state());
            let count = if sample > 20 { state.counter().wrapping_add(1) } else { state.counter().wrapping_sub(1) };
            state.set_counter(count);
            let state = BatchState::from_ptr(view.state());
            if (sample > 20 && state.counter() < 16) || (sample < 5 && state.counter() > -4) {
                return registers(state.as_ptr() as u32, 0);
            }
            state.set_counter(0);
            let state = BatchState::from_ptr(view.state());
            let size = state.size();
            if sample > 20 && size > 1 {
                state.set_size(size >> 1);
                return registers(size >> 1, state.as_ptr() as u32);
            }
            if sample < 5 && size < 1024 {
                state.set_size(size.wrapping_shl(1));
                return registers(size.wrapping_shl(1), state.as_ptr() as u32);
            }
            registers(state.as_ptr() as u32, state.as_ptr() as u32)
        }
    }
}

register_entry!(
    0x006A_CB60,
    fc4p_006acb60,
    remove_retained_object,
    "ret 4",
    "RemoveRetainedObject"
);
unsafe extern "fastcall" fn remove_retained_object(this: *mut u8, frame: *mut u32) {
    unsafe {
        let object = frame.add(4).read_unaligned() as *mut u8;
        let kind = seeded_virtual(
            object,
            0x1C,
            &[],
            word(object, 0),
            frame.add(2).read_unaligned(),
        );
        if kind as u32 != 0x2781_28A0 {
            save_result(frame, kind);
            return;
        }
        let head = pointer(this, 0x1C);
        let mut node = RetainedNode::from_ptr(head).next();
        while node != head && RetainedNode::from_ptr(node).object() != object {
            node = RetainedNode::from_ptr(node).next();
        }
        if node == pointer(this, 0x1C) {
            save_result(frame, registers(node as u32, head as u32));
            return;
        }
        let erased = erase_list_node(this.wrapping_add(0x1C), frame.add(4), node);
        save_result(
            frame,
            seeded_virtual(object, 8, &[], word(object, 0), (erased >> 32) as u32),
        );
    }
}

register_entry!(
    0x006B_5840,
    fc4p_006b5840,
    find_or_insert_integer_value,
    "ret 4",
    "FindOrInsertIntegerValue"
);
unsafe extern "fastcall" fn find_or_insert_integer_value(this: *mut u8, frame: *mut u32) {
    unsafe {
        let key = frame.add(4).read_unaligned() as *const i32;
        let head = pointer(this, 0);
        let mut candidate = head;
        let mut node = pointer(head, 4);
        if !node.is_null() {
            let wanted = key.read_unaligned();
            loop {
                if (word(node, 0x10) as i32) >= wanted {
                    candidate = node;
                    node = pointer(node, 8);
                } else {
                    node = pointer(node, 12);
                }
                if node.is_null() {
                    break;
                }
            }
        }
        if candidate != head && key.read_unaligned() >= word(candidate, 0x10) as i32 {
            save_result(
                frame,
                registers(candidate.wrapping_add(0x14) as u32, candidate as u32),
            );
            return;
        }
        // The default pair is at entry ESP-8, and the iterator result replaces the key argument.
        let pair = frame.add(1);
        pair.write_unaligned(key.read_unaligned() as u32);
        pair.add(1).write_unaligned(0);
        let out = frame.add(4);
        let result = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0069_7530),
            this as u32,
            &[out as u32, candidate as u32, pair as u32],
            pair as u32,
            out as u32,
        );
        save_result(
            frame,
            registers(
                out.read_unaligned().wrapping_add(0x14),
                (result >> 32) as u32,
            ),
        );
    }
}

layout! {
    /// Connection-tool services and the two lists of retained occupants.
    struct ConnectionLists {
        0x048 => occupants / set_occupants: *mut u8,
        0x208 => removal_context / set_removal_context: u32,
        0x368 => initialized / set_initialized: u8,
        0x384 => primary / set_primary: *mut u8,
        0x388 => secondary / set_secondary: *mut u8,
    }
}

game_fn! { #[allow(dead_code)] fn shutdown_connection_base(this: *mut u8) -> u64 = "thiscall" 0x0062_6D30; }
game_fn! { #[allow(dead_code)] fn reset_connection_base(this: *mut u8) -> u64 = "thiscall" 0x0062_5580; }

/// Frees links after the payloads have been released. The owner can replace its header during a free.
unsafe fn free_retained_links(this: *mut u8, offset: usize, mut edx: u32) -> (u32, u32) {
    unsafe {
        let mut header = pointer(this, offset);
        let mut node = RetainedNode::from_ptr(header).next();
        while node != header {
            let freed = node;
            node = RetainedNode::from_ptr(node).next();
            let result = seeded_call(
                crate::abi::Conv::Cdecl,
                crate::abi::target(0x0090_CF63),
                0,
                &[freed as u32],
                freed as u32,
                edx,
            );
            edx = (result >> 32) as u32;
            header = pointer(this, offset);
        }
        let header = pointer(this, offset);
        put(header, 0, header as u32);
        let header = pointer(this, offset);
        put(header, 4, header as u32);
        (header as u32, edx)
    }
}

register_entry!(
    0x0065_7AF0,
    fc4p_00657af0,
    shutdown_connection_lists,
    "ret",
    "ShutdownConnectionLists"
);
unsafe extern "fastcall" fn shutdown_connection_lists(this: *mut u8, frame: *mut u32) {
    unsafe {
        let view = ConnectionLists::from_ptr(this);
        let mut edx = frame.add(2).read_unaligned();
        if view.initialized() == 0 {
            save_result(frame, registers((frame.read_unaligned() & !0xFF) | 1, edx));
            return;
        }
        view.set_initialized(0);
        let header = view.primary();
        let mut node = RetainedNode::from_ptr(header).next();
        while node != header {
            let object = RetainedNode::from_ptr(node).object();
            let released = seeded_virtual(object, 8, &[], word(object, 0), edx);
            edx = (released >> 32) as u32;
            node = RetainedNode::from_ptr(node).next();
        }
        let (_, next_edx) = free_retained_links(this, 0x384, edx);
        edx = next_edx;
        let header = view.secondary();
        let mut node = RetainedNode::from_ptr(header).next();
        let mut eax = header as u32;
        while node != header {
            let object = RetainedNode::from_ptr(node).object();
            let released = seeded_virtual(object, 8, &[], eax, word(object, 0));
            eax = released as u32;
            edx = (released >> 32) as u32;
            node = RetainedNode::from_ptr(node).next();
        }
        let (eax, edx) = free_retained_links(this, 0x388, edx);
        save_result(
            frame,
            seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x0062_6D30),
                this as u32,
                &[],
                eax,
                edx,
            ),
        );
    }
}

register_entry!(
    0x0065_7BE0,
    fc4p_00657be0,
    reset_connection_lists,
    "ret",
    "ResetConnectionLists"
);
unsafe extern "fastcall" fn reset_connection_lists(this: *mut u8, frame: *mut u32) {
    unsafe {
        let view = ConnectionLists::from_ptr(this);
        let mut edx = frame.add(2).read_unaligned();
        let header = view.primary();
        let mut node = RetainedNode::from_ptr(header).next();
        while node != header {
            let object = RetainedNode::from_ptr(node).object();
            let interface = object.wrapping_add(4);
            let detached = seeded_virtual(interface, 0x44, &[0, 0], word(object, 4), edx);
            let mut result = registers(view.occupants() as u32, (detached >> 32) as u32);
            if result as u32 != 0 {
                let model = seeded_virtual(object, 0xC0, &[], result as u32, word(object, 0));
                let model_pointer = model as u32 as *mut u8;
                result = seeded_virtual(
                    model_pointer,
                    0x84,
                    &[],
                    model as u32,
                    word(model_pointer, 0),
                );
                if result as u8 & 1 != 0 {
                    let context = view.removal_context();
                    let service = view.occupants();
                    result = seeded_virtual(
                        service,
                        0x54,
                        &[interface as u32, 0, context],
                        word(service, 0),
                        context,
                    );
                }
            }
            let stopped = seeded_virtual(object, 0x10, &[], word(object, 0), (result >> 32) as u32);
            let released = seeded_virtual(object, 8, &[], stopped as u32, word(object, 0));
            edx = (released >> 32) as u32;
            node = RetainedNode::from_ptr(node).next();
        }
        let (_, next_edx) = free_retained_links(this, 0x384, edx);
        edx = next_edx;
        let header = view.secondary();
        let mut node = RetainedNode::from_ptr(header).next();
        while node != header {
            let object = RetainedNode::from_ptr(node).object();
            let detached =
                seeded_virtual(object.wrapping_add(4), 0x44, &[0, 0], word(object, 4), edx);
            let released = seeded_virtual(object, 8, &[], detached as u32, word(object, 0));
            edx = (released >> 32) as u32;
            node = RetainedNode::from_ptr(node).next();
        }
        let (eax, edx) = free_retained_links(this, 0x388, edx);
        save_result(
            frame,
            seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x0062_5580),
                this as u32,
                &[],
                eax,
                edx,
            ),
        );
    }
}

game_data! { static object_event_manager: *mut u8 = 0x00B4_3D64; }
game_data! { static object_event_server: *mut u8 = 0x00B4_3CCC; }
game_fn! { fn allocate_message(size: u32) -> u64 = "C" 0x0091_33DA; }
game_fn! { #[allow(dead_code)] fn construct_message(this: *mut u8) -> u64 = "thiscall" 0x0091_34D6; }

register_entry!(
    0x006A_9980,
    fc4p_006a9980,
    post_object_event,
    "ret",
    "PostObjectEvent"
);
unsafe extern "fastcall" fn post_object_event(this: *mut u8, frame: *mut u32) {
    unsafe {
        let manager = object_event_manager().read_unaligned();
        seeded_virtual(
            manager,
            0x80,
            &[this as u32],
            word(manager, 0),
            frame.add(2).read_unaligned(),
        );
        let allocation = allocate_message(0x2C);
        let constructed = if allocation as u32 == 0 {
            0
        } else {
            seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x0091_34D6),
                allocation as u32,
                &[],
                allocation as u32,
                (allocation >> 32) as u32,
            )
        };
        let message = constructed as u32 as *mut u8;
        let added = seeded_virtual(message, 4, &[], constructed as u32, word(message, 0));
        let identified = seeded_virtual(
            message,
            0x14,
            &[0x08DA_C3E8],
            word(message, 0),
            (added >> 32) as u32,
        );
        let attached = seeded_virtual(
            message,
            0x30,
            &[this as u32],
            identified as u32,
            word(message, 0),
        );
        seeded_virtual(
            message,
            0x84,
            &[this as u32],
            word(message, 0),
            (attached >> 32) as u32,
        );
        let payload = word(this, 0x10);
        let detailed = seeded_virtual(message, 0x40, &[payload], payload, word(message, 0));
        let configured = seeded_virtual(message, 0x54, &[0], detailed as u32, word(message, 0));
        let server = object_event_server().read_unaligned();
        let posted = seeded_virtual(
            server,
            0x10,
            &[message as u32, 0],
            word(server, 0),
            (configured >> 32) as u32,
        );
        let released = seeded_virtual(message, 8, &[], posted as u32, word(message, 0));
        save_result(frame, released);
    }
}

game_fn! { #[allow(dead_code)] fn lookup_solved_height(this: *mut u8, vertex: u32, output: *mut u32) -> u64 = "thiscall" 0x0065_EB00; }
game_fn! { #[allow(dead_code)] fn link_height_constraints(this: *mut u8, first: u32, second: u32) -> u64 = "thiscall" 0x0065_FDB0; }
game_fn! { #[allow(dead_code, clippy::too_many_arguments)] fn clear_two_ranges(a0: u32, a1: u32, a2: u32, a3: u32, b0: u32, b1: u32, b2: u32, b3: u32, flag: u32) -> u64 = "C" 0x008E_BB10; }
game_fn! { #[allow(dead_code)] fn clear_range_storage(this: *mut u8, first: u32, last: u32) -> u64 = "thiscall" 0x0066_7D40; }
game_fn! { #[allow(dead_code)] fn free_large_range(allocation: *mut u8) -> u64 = "C" 0x009F_015B; }
game_fn! { #[allow(dead_code)] fn copy_thirty_two_byte_records(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 = "C" 0x006E_9150; }
game_data! { static range_allocators: [*mut u8; 9] = 0x00B6_2674; }
game_data! { static float_zero: f32 = 0x00A8_1054; }
game_data! { static height_threshold: f32 = 0x00A8_67A4; }
game_data! { static terrain_city: *mut u8 = 0x00B4_3CEC; }
game_fn! { fn rewind_solved_heights(this: *mut u8) -> u64 = "thiscall" 0x0065_EF70; }
game_fn! { fn next_solved_height(this: *mut u8, vertex: *mut u32, height: *mut u32) -> u64 = "thiscall" 0x0065_EB50; }
game_fn! { fn grow_vertex_vector(this: *mut u8, at: *mut u8, value: *const u32, cookie: *mut u32, count: u32, mode: u32) -> u64 = "thiscall" 0x0051_CA60; }

/// Unsigned division retains the CPU's divide fault for a zero divisor.
fn divide_unsigned(mut numerator: u32, denominator: u32) -> (u32, u32) {
    let remainder: u32;
    unsafe {
        core::arch::asm!(
            "xor edx, edx", "div {denominator}", denominator = in(reg) denominator,
            inout("eax") numerator, out("edx") remainder, options(nomem, nostack),
        );
    }
    (numerator, remainder)
}

register_entry!(
    0x0064_4530,
    fc4p_00644530,
    apply_solved_terrain_heights,
    "ret 4",
    "ApplySolvedTerrainHeights"
);
unsafe extern "fastcall" fn apply_solved_terrain_heights(this: *mut u8, frame: *mut u32) {
    unsafe {
        let mut rect = [word(this, 0x248), word(this, 0x24C), u32::MAX, u32::MAX];
        let changed = frame.add(4).read_unaligned() as *mut u8;
        put(changed, 4, word(changed, 0));
        let iterator = this.wrapping_add(0x188);
        rewind_solved_heights(iterator);
        let mut height = 0u32;
        let vertex = frame.add(4);
        while next_solved_height(iterator, vertex, &mut height) as u8 != 0 {
            let terrain = pointer(this, 0x44);
            let altitude = vcall_st0!(terrain, 0x30, vertex.read_unaligned());
            let difference = height_subtract_from(altitude, f32::from_bits(height)).abs();
            let comparison = difference.compare(crate::x87::F80::from_f32(height_threshold().read_unaligned()));
            if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                vcall!(pointer(this, 0x44), 0xC8 => fn(u32, u32) -> u64, vertex.read_unaligned(), height);
                let end = pointer(changed, 4);
                let mut cookie = vertex.read_unaligned();
                if end != pointer(changed, 8) {
                    if !end.is_null() {
                        put(end, 0, vertex.read_unaligned());
                    }
                    put(changed, 4, word(changed, 4).wrapping_add(4));
                } else {
                    grow_vertex_vector(changed, end, &cookie, vertex, 1, 1);
                }
                // The iterator's vertex slot can be changed by vector growth.
                cookie = vertex.read_unaligned();
                let x = divide_unsigned(cookie, word(this, 0x248)).1;
                let z = divide_unsigned(cookie, word(this, 0x24C)).0;
                if (x as i32) < rect[0] as i32 {
                    rect[0] = x;
                }
                if (x as i32) > rect[2] as i32 {
                    rect[2] = x;
                }
                if (z as i32) < rect[1] as i32 {
                    rect[1] = z;
                }
                if (z as i32) > rect[3] as i32 {
                    rect[3] = z;
                }
            }
        }
        if rect[1] != 0 {
            rect[1] = rect[1].wrapping_sub(1);
        }
        if rect[0] != 0 {
            rect[0] = rect[0].wrapping_sub(1);
        }
        let city = terrain_city().read_unaligned();
        let width = seeded_virtual(city, 0x174, &[], rect[0], word(city, 0));
        if (rect[2] as i32) < width as u32 as i32 {
            rect[2] = rect[2].wrapping_add(1);
        }
        let city = terrain_city().read_unaligned();
        let depth = seeded_virtual(city, 0x178, &[], word(city, 0), (width >> 32) as u32);
        if (rect[3] as i32) < depth as u32 as i32 {
            rect[3] = rect[3].wrapping_add(1);
        }
        let mut result = registers(depth as u32, rect[2]);
        if rect[2] as i32 >= rect[0] as i32 && rect[3] as i32 >= rect[1] as i32 {
            let terrain = pointer(this, 0x44);
            seeded_virtual(terrain, 0x15C, &[], word(terrain, 0), rect[2]);
            let terrain = pointer(this, 0x44);
            let refreshed = seeded_virtual(
                terrain,
                0x110,
                &[1, 1, rect.as_ptr() as u32, 4],
                rect.as_ptr() as u32,
                word(terrain, 0),
            );
            let terrain = pointer(this, 0x44);
            result = seeded_virtual(terrain, 0x160, &[], refreshed as u32, word(terrain, 0));
        }
        save_result(frame, result);
    }
}

// Keep a stored operand in memory until the x87 operation consumes it.
macro_rules! height_float_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: crate::x87::F80, operand: f32) -> crate::x87::F80 {
            let mut output = crate::x87::F80::ZERO;
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _,
                );
            }
            output
        }
    };
}
height_float_op!(height_subtract, "fsub dword ptr [{operand}]");
height_float_op!(height_subtract_from, "fsubr dword ptr [{operand}]");

// The boundary returns an unrounded x87 value and retains the integer result registers.
macro_rules! height_entry {
    ($address:literal, $export:ident, $body:ident, $name:literal, $adjust:literal) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "sub esp, 12",
                    "lea edx, [esp + 12]", "push esp", "call {body}",
                    "fld tbyte ptr [esp]", "add esp, 12", "pop eax", "pop ecx", "pop edx", "ret 4",
                    body = sym $body,
                );
            }
        }
        unsafe extern "fastcall" fn $body(this: *mut u8, frame: *mut u32, output: *mut crate::x87::F80) {
            unsafe { solved_or_terrain_height(this, frame, output, $adjust) }
        }
    };
}

height_entry!(
    0x0064_44F0,
    fc4p_006444f0,
    get_solved_or_terrain_height,
    "GetSolvedOrTerrainHeight",
    false
);
height_entry!(
    0x0065_7790,
    fc4p_00657790,
    get_solved_or_offset_height,
    "GetSolvedOrOffsetTerrainHeight",
    true
);

unsafe fn solved_or_terrain_height(this: *mut u8, frame: *mut u32, output: *mut crate::x87::F80, adjust: bool) {
    unsafe {
        let argument = frame.add(4);
        let vertex = argument.read_unaligned();
        let found = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0065_EB00),
            this.wrapping_add(0x124) as u32,
            &[vertex, argument as u32],
            argument as u32,
            frame.add(2).read_unaligned(),
        );
        if found as u8 != 0 {
            output.write_unaligned(crate::x87::F80::from_f32(f32::from_bits(argument.read_unaligned())));
            save_result(frame, found);
        } else {
            let terrain = pointer(this, 0x44);
            let args = [vertex];
            let mut call = crate::abi::RawCall::new(
                crate::abi::Conv::Thiscall,
                crate::abi::vslot(terrain, 0x30),
                terrain as u32,
                &args,
            );
            call.eax = found as u32;
            call.edx = word(terrain, 0);
            call.take_st0 = 1;
            crate::abi::call_raw(&mut call);
            let value = if adjust {
                height_subtract(call.st0, f32::from_bits(word(this, 0x36C)))
            } else {
                call.st0
            };
            output.write_unaligned(value);
            save_result(frame, registers(call.eax, call.edx));
        }
    }
}

port! {
    /// Stores a direction scaled by its largest absolute component, along with the supplied origin and kind.
    #[port(address = 0x0064_9820, owner = "shared", name = "SetDominantAxisDirection")]
    pub unsafe extern "thiscall" fn fc4p_00649820(this: *mut u8, x: u32, z: u32, dx: f32, dz: f32, kind: u32) -> u64 {
        use crate::x87::F80;
        unsafe {
            let ax = F80::from_f32(dx).abs();
            let az = F80::from_f32(dz).abs();
            let comparison = ax.compare(az);
            if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                put(this, 0x10, 1f32.to_bits());
                put(this, 0x14, (az / ax).to_f32().to_bits());
            } else {
                let zero = F80::from_f32(float_zero().read_unaligned());
                let value = if az.compare(zero).equal() { 0.0 } else { (ax / az).to_f32() };
                put(this, 0x10, value.to_bits());
                put(this, 0x14, 1f32.to_bits());
            }
            if F80::from_f32(dx).compare(F80::from_f32(float_zero().read_unaligned())).less() {
                put(this, 0x10, (-F80::from_f32(f32::from_bits(word(this, 0x10)))).to_f32().to_bits());
            }
            if F80::from_f32(dz).compare(F80::from_f32(float_zero().read_unaligned())).less() {
                put(this, 0x14, (-F80::from_f32(f32::from_bits(word(this, 0x14)))).to_f32().to_bits());
            }
            put(this, 0, kind);
            put(this, 4, 0);
            put(this, 8, x);
            put(this, 12, z);
            registers(z, x)
        }
    }
}

register_entry!(
    0x0065_7860,
    fc4p_00657860,
    link_cell_height_edges,
    "ret 4",
    "LinkCellHeightEdges"
);
unsafe extern "fastcall" fn link_cell_height_edges(this: *mut u8, frame: *mut u32) {
    unsafe {
        let found = lookup_network_cell(this, frame.add(4).read_unaligned());
        let cell = found as u32 as *mut u8;
        if cell.is_null() {
            save_result(frame, found);
            return;
        }
        let mut eax = (cell as u32 & !0xFF) | u32::from(cell.add(0x53).read());
        let mut edx = (found >> 32) as u32;
        if eax as u8 != 0 {
            save_result(frame, registers(eax, edx));
            return;
        }
        let predefined = cell.add(0x57).read();
        eax = (eax & !0xFF) | u32::from(predefined);
        let kind;
        if predefined != 0 {
            eax = word(cell, 0xB8);
            edx = word(this, 0x98);
            kind = word(edx as *mut u8, eax.wrapping_mul(12) as usize);
        } else {
            let occupant = pointer(cell, 0x0C);
            if occupant.is_null() {
                save_result(frame, registers(eax, edx));
                return;
            }
            let classified = seeded_virtual(occupant, 0x5C, &[], word(occupant, 0), edx);
            kind = classified as u32;
            let occupant = pointer(cell, 0x0C);
            let mut first = core::mem::MaybeUninit::<u32>::uninit();
            let got = seeded_virtual(
                occupant,
                0xEC,
                &[first.as_mut_ptr() as u32, frame.add(4) as u32],
                first.as_mut_ptr() as u32,
                word(occupant, 0),
            );
            eax = got as u32;
            edx = (got >> 32) as u32;
        }
        if [0, 0x200, 0x300, 0x800, 0xA00, 0x1400, 0x4B00, 0x14E00].contains(&kind) {
            save_result(frame, registers(eax, edx));
            return;
        }
        let bias = if cell.add(0x56).read() != 0 { 100_000u32 } else { 0 };
        for edge in 0usize..3 {
            let second = word(cell, 0xA8 + (edge + 1) * 4).wrapping_add(bias);
            let first = word(cell, 0xA8 + edge * 4).wrapping_add(bias);
            if first != second && first as i32 >= 0 && second as i32 >= 0 {
                let result = seeded_call(
                    crate::abi::Conv::Thiscall,
                    crate::abi::target(0x0065_FDB0),
                    this.wrapping_add(0x124) as u32,
                    &[first, second],
                    second,
                    edx,
                );
                edx = (result >> 32) as u32;
            }
        }
        save_result(frame, registers(3, edx));
    }
}

register_entry!(
    0x0065_7A30,
    fc4p_00657a30,
    refresh_occupant_flags,
    "ret 8",
    "RefreshOccupantFlags"
);
unsafe extern "fastcall" fn refresh_occupant_flags(_this: *mut u8, frame: *mut u32) {
    unsafe {
        let occupant = frame.add(4).read_unaligned() as *mut u8;
        let mut interface = core::ptr::null_mut::<u8>();
        let mut result = seeded_virtual(
            occupant,
            0,
            &[0x4A10_AFC2, &mut interface as *mut _ as u32],
            word(occupant, 0),
            frame.add(2).read_unaligned(),
        );
        if result as u8 != 0 {
            for (flag, eax_table) in [(0x0100_0000, false), (0x0200_0000, true), (0x0400_0000, false)] {
                result = seeded_virtual(
                    occupant,
                    0x2C,
                    &[flag],
                    if eax_table { word(occupant, 0) } else { result as u32 },
                    if eax_table {
                        (result >> 32) as u32
                    } else {
                        word(occupant, 0)
                    },
                );
            }
            let mut second = core::mem::MaybeUninit::<u8>::uninit();
            let mut third = core::mem::MaybeUninit::<u8>::uninit();
            result = seeded_virtual(
                interface,
                0x170,
                &[
                    frame.add(4) as u32,
                    second.as_mut_ptr() as u32,
                    third.as_mut_ptr() as u32,
                ],
                word(interface, 0),
                third.as_mut_ptr() as u32,
            );
            let first = frame.add(4).read_unaligned() as u8;
            result = (result & !0xFF) | u64::from(first);
            if first != 0 {
                result = seeded_virtual(occupant, 0x28, &[0x0100_0000], word(occupant, 0), (result >> 32) as u32);
            }
            let second = second.assume_init();
            result = (result & !0xFF) | u64::from(second);
            if second == 0 {
                result = seeded_virtual(occupant, 0x28, &[0x0200_0000], result as u32, word(occupant, 0));
            }
            let third = third.assume_init();
            result = (result & !0xFF) | u64::from(third);
            if third == 0 {
                result = seeded_virtual(occupant, 0x28, &[0x0400_0000], word(occupant, 0), (result >> 32) as u32);
            }
        }
        if !interface.is_null() {
            result = seeded_virtual(interface, 8, &[], result as u32, word(interface, 0));
        }
        save_result(frame, result);
    }
}

register_entry!(
    0x0066_8AB0,
    fc4p_00668ab0,
    destroy_paired_ranges,
    "ret",
    "DestroyPairedRanges"
);
unsafe extern "fastcall" fn destroy_paired_ranges(this: *mut u8, frame: *mut u32) {
    unsafe {
        let args = [
            word(this, 0),
            word(this, 4),
            word(this, 8),
            word(this, 12),
            word(this, 16),
            word(this, 20),
            word(this, 24),
            word(this, 28),
            0,
        ];
        let cleared = seeded_call(
            crate::abi::Conv::Cdecl,
            crate::abi::target(0x008E_BB10),
            args[3],
            &args,
            args.as_ptr() as u32,
            args[1],
        );
        let allocation = pointer(this, 0x20);
        if allocation.is_null() {
            save_result(frame, registers(0, (cleared >> 32) as u32));
            return;
        }
        let first = word(this, 0x0C);
        let last = word(this, 0x1C).wrapping_add(4);
        let cleared = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0066_7D40),
            this as u32,
            &[first, last],
            first,
            last,
        );
        let allocation = pointer(this, 0x20);
        if allocation.is_null() {
            save_result(frame, cleared);
            return;
        }
        let tag = allocation.wrapping_sub(1).read();
        let header = allocation.wrapping_sub(4);
        let result = if tag < 9 {
            let allocator = range_allocators().cast::<*mut u8>().add(tag as usize).read_unaligned();
            seeded_virtual(allocator, 4, &[header as u32], header as u32, word(allocator, 0))
        } else {
            seeded_call(
                crate::abi::Conv::Cdecl,
                crate::abi::target(0x009F_015B),
                tag as u32,
                &[header as u32],
                header as u32,
                (cleared >> 32) as u32,
            )
        };
        save_result(frame, result);
    }
}

register_entry!(
    0x0069_15E0,
    fc4p_006915e0,
    reallocate_thirty_two_byte_vector,
    "ret 20",
    "ReallocateThirtyTwoByteVector"
);
unsafe extern "fastcall" fn reallocate_thirty_two_byte_vector(this: *mut u8, frame: *mut u32) {
    unsafe {
        let at = frame.add(4).read_unaligned() as *mut u8;
        let value = frame.add(5).read_unaligned() as *mut u8;
        let count = frame.add(7).read_unaligned();
        let length = (word(this, 4).wrapping_sub(word(this, 0)) as i32 >> 5) as u32;
        let capacity = length.wrapping_add(length.max(count));
        let allocation = if capacity == 0 {
            core::ptr::null_mut()
        } else {
            allocate_storage(capacity.wrapping_mul(32))
        };
        let mut source = pointer(this, 0);
        let mut out = allocation;
        let mut edx = source as u32;
        while source != at {
            if !out.is_null() {
                for offset in (0..32).step_by(4) {
                    put(out, offset, word(source, offset));
                }
            }
            source = source.wrapping_add(32);
            out = out.wrapping_add(32);
            edx = source as u32;
        }
        for _ in 0..count {
            if !out.is_null() {
                for offset in (0..32).step_by(4) {
                    put(out, offset, word(value, offset));
                }
            }
            out = out.wrapping_add(32);
        }
        if count > 1 {
            edx = 0;
        }
        if frame.add(8).read_unaligned() as u8 == 0 {
            let end = pointer(this, 4);
            let cookie = frame.add(8);
            let copied = seeded_call(
                crate::abi::Conv::Cdecl,
                crate::abi::target(0x006E_9150),
                cookie as u32,
                &[at as u32, end as u32, out as u32, cookie as u32],
                at as u32,
                end as u32,
            );
            out = copied as u32 as *mut u8;
            edx = (copied >> 32) as u32;
        }
        let original = pointer(this, 0);
        if !original.is_null() {
            let freed = seeded_call(
                crate::abi::Conv::Cdecl,
                crate::abi::target(0x0090_CF63),
                frame as u32,
                &[original as u32],
                original as u32,
                edx,
            );
            edx = (freed >> 32) as u32;
        }
        put(this, 4, out as u32);
        put(this, 8, (allocation as u32).wrapping_add(capacity.wrapping_mul(32)));
        put(this, 0, allocation as u32);
        save_result(frame, registers(allocation as u32, edx));
    }
}

game_fn! { fn read_entry_pair(this: *mut u8, key: *mut u32, value: *mut u32) -> u64 = "thiscall" 0x0067_E0B0; }
game_fn! { fn compare_entry_key(first: *const u8, second: *const u8) -> u64 = "C" 0x00A0_4487; }
game_fn! { fn parse_entry_integer(text: *const u8) -> u64 = "C" 0x0090_FE93; }
game_fn! { #[allow(dead_code)] fn interpret_boolean_entry() -> u64 = "C" 0x0067_DFD0; }
game_fn! { #[allow(dead_code)] fn interpret_entry_value(output: *mut u8) -> u64 = "C" 0x0067_E780; }

unsafe fn make_entry_string() -> [u32; 5] {
    unsafe {
        let allocation = allocate_storage(8);
        allocation.write(0);
        [
            0x00A8_0810,
            allocation as u32,
            allocation as u32,
            allocation.wrapping_add(8) as u32,
            0,
        ]
    }
}

unsafe fn discard_entry_strings(key: &mut [u32; 5], value: &mut [u32; 5], mut result: u64) -> u64 {
    unsafe {
        for string in [value, key] {
            let allocation = string[1] as *mut u8;
            string[0] = 0x00A8_0810;
            result = registers(allocation as u32, (result >> 32) as u32);
            if !allocation.is_null() {
                result = seeded_call(
                    crate::abi::Conv::Cdecl,
                    crate::abi::target(0x0090_CF63),
                    string.as_mut_ptr() as u32,
                    &[allocation as u32],
                    allocation as u32,
                    (result >> 32) as u32,
                );
            }
        }
        result
    }
}

port! {
    /// Reads a named boolean entry and releases the two temporary strings, retaining the interpreter's low byte.
    #[port(address = 0x0067_E140, owner = "shared", name = "ReadNamedBooleanEntry")]
    pub unsafe extern "C" fn fc4p_0067e140(parser: *mut u8, expected: *mut u8) -> u64 {
        unsafe {
            let mut key = make_entry_string();
            let mut value = make_entry_string();
            let mut result = read_entry_pair(parser, key.as_mut_ptr(), value.as_mut_ptr());
            let mut answer = 0;
            if result as u8 != 0 {
                result = compare_entry_key(key[1] as *const u8, pointer(expected, 4));
                if result as u32 == 0 {
                    result = seeded_call(crate::abi::Conv::Cdecl, crate::abi::target(0x0067_DFD0),
                        parser as u32, &[], value.as_mut_ptr() as u32, (result >> 32) as u32);
                    answer = result as u8;
                }
            }
            (discard_entry_strings(&mut key, &mut value, result) & !0xFF) | u64::from(answer)
        }
    }
}

port! {
    /// Reads a named integer entry, writing the destination only after the key matches.
    #[port(address = 0x0067_E250, owner = "shared", name = "ReadNamedIntegerEntry")]
    pub unsafe extern "C" fn fc4p_0067e250(parser: *mut u8, expected: *mut u8, output: *mut u32) -> u64 {
        unsafe {
            let mut key = make_entry_string();
            let mut value = make_entry_string();
            let mut result = read_entry_pair(parser, key.as_mut_ptr(), value.as_mut_ptr());
            let mut answer = 0;
            if result as u8 != 0 {
                result = compare_entry_key(key[1] as *const u8, pointer(expected, 4));
                if result as u32 == 0 {
                    result = parse_entry_integer(value[1] as *const u8);
                    output.write_unaligned(result as u32);
                    answer = 1;
                }
            }
            (discard_entry_strings(&mut key, &mut value, result) & !0xFF) | answer
        }
    }
}

port! {
    /// Reads an integer key and delegates the value conversion, retaining the key even if conversion fails.
    #[port(address = 0x0067_F0D0, owner = "shared", name = "ReadIntegerKeyAndValue")]
    pub unsafe extern "C" fn fc4p_0067f0d0(parser: *mut u8, key_output: *mut u32, value_output: *mut u8) -> u64 {
        unsafe {
            let mut key = make_entry_string();
            let mut value = make_entry_string();
            let mut result = read_entry_pair(parser, key.as_mut_ptr(), value.as_mut_ptr());
            let mut answer = 0;
            if result as u8 != 0 {
                result = parse_entry_integer(key[1] as *const u8);
                key_output.write_unaligned(result as u32);
                result = seeded_call(crate::abi::Conv::Cdecl, crate::abi::target(0x0067_E780),
                    key_output as u32, &[value_output as u32], value.as_mut_ptr() as u32, value_output as u32);
                answer = u64::from(result as u8 != 0);
            }
            (discard_entry_strings(&mut key, &mut value, result) & !0xFF) | answer
        }
    }
}

game_data! { static occupant_list_allocator: *mut u8 = 0x00B2_07E4; }
game_fn! { #[allow(dead_code)] fn get_framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_queried_list_allocator(this: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { fn clear_occupant_list(this: *mut u32) -> u64 = "thiscall" 0x0041_3E60; }

unsafe fn ensure_occupant_list_allocator(temporary: *mut *mut u8, mut edx: u32) -> (*mut u8, u32) {
    unsafe {
        if occupant_list_allocator().read_unaligned().is_null() {
            temporary.write_unaligned(core::ptr::null_mut());
            let got = seeded_call(crate::abi::Conv::Cdecl, crate::abi::target(0x0087_93EC), 0, &[], 0, edx);
            let framework = got as u32 as *mut u8;
            let mut eax = got as u32;
            if !framework.is_null() {
                eax = seeded_virtual(
                    framework,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, temporary as u32],
                    framework as u32,
                    word(framework, 0),
                ) as u32;
            }
            occupant_list_allocator().write_unaligned(temporary.read_unaligned());
            edx = (seeded_call(
                crate::abi::Conv::Thiscall,
                crate::abi::target(0x0041_1870),
                temporary as u32,
                &[],
                eax,
                temporary.read_unaligned() as u32,
            ) >> 32) as u32;
        }
        (occupant_list_allocator().read_unaligned(), edx)
    }
}

register_entry!(
    0x0063_9170,
    fc4p_00639170,
    find_blocking_network_cell,
    "ret 12",
    "FindBlockingNetworkCell"
);
unsafe extern "fastcall" fn find_blocking_network_cell(this: *mut u8, frame: *mut u32) {
    unsafe {
        let mut temporary = core::ptr::null_mut();
        let (allocator, _) = ensure_occupant_list_allocator(&mut temporary, frame.add(2).read_unaligned());
        let sentinel = vcall!(allocator, 0x0C => fn(u32) -> *mut u8, 12);
        put(sentinel, 0, sentinel as u32);
        put(sentinel, 4, sentinel as u32);
        let mut list = [sentinel as u32; 2];
        let count = frame.add(4).read_unaligned() as *mut i32;
        let context = frame.add(5).read_unaligned();
        let mut found = false;
        if frame.add(6).read_unaligned() as u8 == 0 {
            let mut segment = 0i32;
            let mut offset = 0usize;
            while segment < count.read_unaligned() {
                let row = pointer(this, 0x54).wrapping_add(offset);
                if word(row, 8) != 3 {
                    let mut index = word(row, 0);
                    if index <= word(row, 4) {
                        loop {
                            let coordinates = pointer(this, 0x60).wrapping_add(index.wrapping_mul(8) as usize);
                            let result = vcall!(this, 0x10C => fn(u32, u32, u32) -> u64,
                                word(coordinates, 0), word(coordinates, 4), context);
                            if result as u8 != 0 {
                                count.write_unaligned(segment);
                                found = true;
                                break;
                            }
                            index = index.wrapping_add(1);
                            if index > word(pointer(this, 0x54), offset + 4) {
                                break;
                            }
                        }
                    }
                }
                if found {
                    break;
                }
                segment = segment.wrapping_add(1);
                offset = offset.wrapping_add(12);
            }
        }
        if !found {
            let mut extra = pointer(this, 0xF0);
            let end = pointer(this, 0xF4);
            while extra != end {
                if vcall!(this, 0x10C => fn(u32, u32, u32) -> u64, word(extra, 0), word(extra, 4), word(extra, 12))
                    as u8
                    != 0
                {
                    count.write_unaligned(0);
                    break;
                }
                extra = extra.wrapping_add(20);
            }
        }
        clear_occupant_list(list.as_mut_ptr());
        let (allocator, _) = ensure_occupant_list_allocator(frame.add(6).cast(), 0);
        let result = seeded_virtual(allocator, 0x10, &[list[1]], word(allocator, 0), list[1]);
        save_result(frame, result);
    }
}

register_entry!(
    0x0063_92F0,
    fc4p_006392f0,
    check_occupants_at_point,
    "ret 12",
    "CheckOccupantsAtPoint"
);
unsafe extern "fastcall" fn check_occupants_at_point(this: *mut u8, frame: *mut u32) {
    unsafe {
        let mut temporary = core::ptr::null_mut();
        let (allocator, edx) = ensure_occupant_list_allocator(&mut temporary, frame.add(2).read_unaligned());
        let allocated = seeded_virtual(allocator, 0x0C, &[12], word(allocator, 0), edx);
        let sentinel = allocated as u32 as *mut u8;
        put(sentinel, 0, sentinel as u32);
        put(sentinel, 4, sentinel as u32);
        let mut list = sentinel as u32;
        let point = [frame.add(4).read_unaligned(), frame.add(5).read_unaligned()];
        let service = pointer(this, 0x48);
        let mut result = seeded_virtual(
            service,
            0xA0,
            &[
                &mut list as *mut u32 as u32,
                point.as_ptr() as u32,
                point.as_ptr() as u32,
                0,
                u32::MAX,
            ],
            word(service, 0),
            &mut list as *mut u32 as u32,
        );
        let end = list as *mut u8;
        let mut node = pointer(end, 0);
        let mut blocked = false;
        while node != end {
            let occupant = pointer(node, 8);
            result = seeded_virtual(occupant, 0x1C, &[], word(occupant, 0), (result >> 32) as u32);
            let kind = result as u32;
            if kind == 0x6A11_BD2B {
                blocked = true;
            } else if kind == 0xC772_BF98 {
                let mut network = core::ptr::null_mut::<u8>();
                result = seeded_virtual(
                    occupant,
                    0,
                    &[0xA821_EF94, &mut network as *mut *mut u8 as u32],
                    &mut network as *mut *mut u8 as u32,
                    word(occupant, 0),
                );
                if result as u8 != 0 {
                    result = seeded_virtual(network, 0x44, &[], result as u32, word(network, 0));
                    if result as u8 != 0 {
                        result = seeded_virtual(network, 0x20, &[0x0020_0000], word(network, 0), (result >> 32) as u32);
                        if result as u8 == 0 {
                            blocked = true;
                        } else {
                            let context = frame.add(6).read_unaligned();
                            result = seeded_virtual(network, 0x58, &[context], context, word(network, 0));
                            if result as u8 == 0 {
                                blocked = true;
                            }
                        }
                    }
                }
                if !network.is_null() {
                    result = seeded_virtual(network, 8, &[], result as u32, word(network, 0));
                }
            }
            result = seeded_virtual(occupant, 8, &[], word(occupant, 0), (result >> 32) as u32);
            node = pointer(node, 0);
        }
        result = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0041_3E60),
            &mut list as *mut u32 as u32,
            &[],
            result as u32,
            (result >> 32) as u32,
        );
        let sentinel = list;
        let (allocator, edx) = ensure_occupant_list_allocator(frame.add(4).cast(), (result >> 32) as u32);
        let result = seeded_virtual(allocator, 0x10, &[sentinel], word(allocator, 0), edx);
        save_result(frame, (result & !0xFF) | u64::from(blocked));
    }
}

game_fn! { #[allow(dead_code)] fn copy_network_cells(this: *mut u8, output: *mut u32) -> u64 = "thiscall" 0x0064_7E60; }

register_entry!(
    0x0065_8710,
    fc4p_00658710,
    refresh_underground_corner_heights,
    "ret",
    "RefreshUndergroundCornerHeights"
);
unsafe extern "fastcall" fn refresh_underground_corner_heights(this: *mut u8, frame: *mut u32) {
    unsafe {
        let mut cells = [0u32; 3];
        let mut result = seeded_call(
            crate::abi::Conv::Thiscall,
            crate::abi::target(0x0064_7E60),
            this.wrapping_add(0x1C) as u32,
            &[cells.as_mut_ptr() as u32],
            cells.as_mut_ptr() as u32,
            frame.add(2).read_unaligned(),
        );
        let end = cells[1] as *mut u8;
        let mut cell = cells[0] as *mut u8;
        while cell != end {
            if cell.add(0x54).read() == 0 || cell.add(0x10).read() != 0 {
                let key = word(cell, 4).wrapping_shl(16).wrapping_add(word(cell, 0));
                let first = pointer(this, 0xA8);
                let count = (word(this, 0xAC).wrapping_sub(first as u32) as i32 >> 2) as u32;
                let mut node = pointer(first, divide_unsigned(key, count).1.wrapping_mul(4) as usize);
                while !node.is_null() && word(node, 4) != key {
                    node = pointer(node, 0);
                }
                result = registers(first as u32, node as u32);
                if !node.is_null() {
                    let mut occupant = core::ptr::null_mut::<u8>();
                    let service = seeded_virtual(this, 0x144, &[], first as u32, word(this, 0));
                    result = seeded_call(
                        crate::abi::Conv::Cdecl,
                        crate::abi::target(0x0065_84D0),
                        word(cell, 4),
                        &[
                            word(cell, 0),
                            word(cell, 4),
                            &mut occupant as *mut *mut u8 as u32,
                            service as u32,
                        ],
                        &mut occupant as *mut *mut u8 as u32,
                        word(cell, 0),
                    );
                    if !occupant.is_null() {
                        result =
                            seeded_virtual(occupant, 0x20, &[0x0020_0000], word(occupant, 0), (result >> 32) as u32);
                        if result as u8 == 0 {
                            let mut old = [0u32; 4];
                            result = seeded_virtual(
                                occupant,
                                0x88,
                                &[old.as_mut_ptr() as u32],
                                old.as_mut_ptr() as u32,
                                word(occupant, 0),
                            );
                            let mut desired = [0u32; 4];
                            let mut changed = false;
                            for i in 0..4 {
                                let args = [word(cell, 0xA8 + i * 4)];
                                let mut call = crate::abi::RawCall::new(
                                    crate::abi::Conv::Thiscall,
                                    crate::abi::vslot(this, 0x124),
                                    this as u32,
                                    &args,
                                );
                                call.eax = args[0];
                                call.edx = word(this, 0);
                                call.take_st0 = 1;
                                crate::abi::call_raw(&mut call);
                                desired[i] = call.st0.to_f32().to_bits();
                                let comparison = height_subtract(call.st0, f32::from_bits(old[i]))
                                    .abs()
                                    .compare(crate::x87::F80::from_f32(height_threshold().read_unaligned()));
                                if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                                    changed = true;
                                }
                                result = registers(call.eax, call.edx);
                            }
                            if changed {
                                result = seeded_virtual(
                                    occupant,
                                    0x150,
                                    &[desired.as_ptr() as u32],
                                    word(occupant, 0),
                                    desired.as_ptr() as u32,
                                );
                                result = seeded_virtual(occupant, 0x158, &[1], result as u32, word(occupant, 0));
                            }
                        }
                        result = seeded_virtual(occupant, 8, &[], word(occupant, 0), (result >> 32) as u32);
                    }
                }
            }
            cell = cell.wrapping_add(0xBC);
            result = registers(end as u32, (result >> 32) as u32);
        }
        // The descriptor is read again after the update callbacks, then every copied reference is released.
        let mut cell = cells[0] as *mut u8;
        let last = cells[1] as *mut u8;
        while cell != last {
            let occupant = pointer(cell, 0x0C);
            if !occupant.is_null() {
                result = seeded_virtual(occupant, 8, &[], result as u32, word(occupant, 0));
            }
            cell = cell.wrapping_add(0xBC);
            result = registers(cell as u32, (result >> 32) as u32);
        }
        let first = cells[0] as *mut u8;
        if !first.is_null() {
            result = seeded_call(
                crate::abi::Conv::Cdecl,
                crate::abi::target(0x0090_CF63),
                first as u32,
                &[first as u32],
                first as u32,
                (result >> 32) as u32,
            );
        }
        save_result(frame, result);
    }
}

layout! {
    /// Segment records, cell coordinates and grading parameters of a network tool.
    struct PlaneWindows {
        0x054 => segments / set_segments: *mut u8,
        0x060 => coordinates / set_coordinates: *mut u8,
        0x078 => count / set_count: i32,
        0x238 => cell_size / set_cell_size: f32,
        0x2B0 => tolerance / set_tolerance: f32,
    }
}

layout! {
    /// An inclusive cell-index range and its segment kind; zero kinds receive plane windows.
    struct PlaneSegment {
        0x00 => first / set_first: u32,
        0x04 => last / set_last: u32,
        0x08 => kind / set_kind: u32,
    }
}

layout! {
    /// An anchor span's signed segment indices and the two endpoint payloads.
    struct PlaneAnchor {
        0x00 => first / set_first: i32,
        0x04 => last / set_last: i32,
        0x08 => near / set_near: u32,
        0x0C => far / set_far: u32,
    }
}

layout! {
    /// A cell's unsigned position, four terrain heights, allowed windows and preferred windows.
    /// Window pairs are accessed individually so a retained endpoint receives no write.
    struct PlaneCell {
        0x00 => x / set_x: u32,
        0x04 => z / set_z: u32,
        0x58 => terrain / set_terrain: [f32; 4],
        0x68 => allowed / set_allowed: [[f32; 2]; 4],
        0x88 => preferred / set_preferred: [[f32; 2]; 4],
    }
}

game_fn! { #[allow(dead_code)] fn build_segment_plane(this: *mut u8, output: *mut f32, first: i32, last: i32, near: u32, far: u32) -> u64 = "thiscall" 0x0063_7780; }
// This entry consumes the stack frame and flags prepared by the preceding prefix.
game_fn! { #[allow(dead_code)] fn continue_connection_initialization(this: *mut u8, first: u32, second: u32) -> u32 = "thiscall" 0x0065_471D; }
game_data! { static plane_negative_one: f32 = 0x00A8_FE78; }
game_data! { static plane_unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static plane_interval_epsilon: f32 = 0x00AA_8A74; }
game_data! { static plane_corner_x: [u32; 4] = 0x00AA_8B30; }
game_data! { static plane_corner_z: [u32; 4] = 0x00AA_8B20; }
game_data! { #[allow(dead_code)] static connection_services_initialized: u8 = 0x00B4_6838; }

height_float_op!(plane_add, "fadd dword ptr [{operand}]");
height_float_op!(plane_multiply, "fmul dword ptr [{operand}]");
height_float_op!(plane_divide, "fdiv dword ptr [{operand}]");

/// Compares against a stored single without first quieting its NaN payload.
fn plane_compare(value: crate::x87::F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand,
            out("ax") status, out("st(0)") _,
        );
    }
    status
}

fn plane_less(status: u16) -> bool {
    status & 0x4500 == 0x0100
}

/// Loads the wrapped coordinate as signed, then corrects its unsigned high half on the x87.
unsafe fn plane_coordinate(value: u32) -> crate::x87::F80 {
    let value_signed = value as i32;
    let value = crate::x87::F80::from_i32(value_signed);
    if value_signed < 0 {
        unsafe { plane_add(value, plane_unsigned_bias().read_unaligned()) }
    } else {
        value
    }
}

port! {
    /// Intersects four corner windows with a cached segment plane, retaining the terrain height.
    /// The plane output occupies the original four stack words, including on an early zero-plane return.
    #[port(address = 0x0063_7B90, owner = "shared", name = "ApplySegmentPlaneWindows", naked)]
    pub unsafe extern "C" fn fc4p_00637b90() {
        core::arch::naked_asm!(
            "sub esp, 0x48", "push edx", "push ecx", "push eax", "mov edx, esp",
            "call {body}", "pop eax", "pop ecx", "pop edx", "add esp, 0x48", "ret 4",
            body = sym apply_segment_plane_windows,
        );
    }
}

unsafe extern "fastcall" fn apply_segment_plane_windows(this: *mut u8, frame: *mut u32) {
    use crate::x87::F80;
    unsafe {
        let tool = PlaneWindows::from_ptr(this);
        let mut count = tool.count();
        if count <= 0 {
            return;
        }
        let anchors = ByteRange::from_ptr(frame.add(22).read_unaligned() as *mut u8);
        let plane = frame.add(17).cast::<f32>();
        let (mut index, mut offset) = (0i32, 0u32);
        let (mut last_anchor, mut cached) = (-1i32, false);
        let mut inverse = 0.0f32;
        let (mut eax, mut edx);
        loop {
            eax = tool.segments() as u32;
            edx = PlaneSegment::from_ptr((eax as *mut u8).wrapping_add(offset as usize)).kind();
            if edx == 0 {
                if index > last_anchor || !cached {
                    let mut record = anchors.first();
                    let end = anchors.last();
                    let (mut first_anchor, mut near, mut far) = (-1i32, 0u32, 0u32);
                    last_anchor = i32::MAX;
                    while record != end {
                        let anchor = PlaneAnchor::from_ptr(record);
                        let before = anchor.last();
                        if before < index && first_anchor < before {
                            first_anchor = before;
                            near = anchor.far();
                        }
                        let after = anchor.first();
                        if index < after && after < last_anchor {
                            last_anchor = after;
                            far = anchor.near();
                        }
                        record = record.wrapping_add(16);
                    }
                    if first_anchor == -1 {
                        first_anchor = index;
                        near = 0;
                    }
                    if last_anchor == i32::MAX {
                        last_anchor = count.wrapping_sub(1);
                        far = 0;
                    }
                    let result = seeded_call(
                        crate::abi::Conv::Thiscall,
                        crate::abi::target(0x0063_7780),
                        this as u32,
                        &[plane as u32, first_anchor as u32, last_anchor as u32, near, far],
                        last_anchor as u32,
                        first_anchor as u32,
                    );
                    let zero = float_zero().read_unaligned();
                    let status: u16;
                    core::arch::asm!(
                        "fld dword ptr [{zero}]", "fld dword ptr [{coefficient}]", "fucompp", "fnstsw ax",
                        zero = in(reg) &zero, coefficient = in(reg) plane.add(1),
                        out("ax") status, out("st(0)") _, out("st(1)") _,
                    );
                    if status & 0x4500 == 0x4000 {
                        save_result(
                            frame,
                            registers((result as u32 & 0xFFFF_0000) | u32::from(status), (result >> 32) as u32),
                        );
                        return;
                    }
                    inverse = plane_divide(
                        F80::from_f32(plane_negative_one().read_unaligned()),
                        plane.add(1).read_unaligned(),
                    )
                    .to_f32();
                    cached = true;
                }
                edx = tool.segments() as u32;
                let segment = (edx as *mut u8).wrapping_add(offset as usize);
                let mut cell_index = PlaneSegment::from_ptr(segment).first();
                eax = segment as u32;
                if cell_index <= PlaneSegment::from_ptr(segment).last() {
                    loop {
                        let coordinate = tool.coordinates().wrapping_add(cell_index.wrapping_mul(8) as usize);
                        let x = word(coordinate, 0);
                        let key = word(coordinate, 4).wrapping_shl(16).wrapping_add(x);
                        let cell = seeded_call(
                            crate::abi::Conv::Thiscall,
                            crate::abi::target(0x0063_3220),
                            this as u32,
                            &[key],
                            key,
                            x,
                        ) as u32 as *mut u8;
                        let cell_view = PlaneCell::from_ptr(cell);
                        for corner in 0..4usize {
                            let x = plane_corner_x()
                                .cast::<u32>()
                                .add(corner)
                                .read_unaligned()
                                .wrapping_add(cell_view.x());
                            let tx = plane_multiply(
                                plane_multiply(plane_coordinate(x), tool.cell_size()),
                                plane.read_unaligned(),
                            );
                            let z = plane_corner_z()
                                .cast::<u32>()
                                .add(corner)
                                .read_unaligned()
                                .wrapping_add(cell_view.z());
                            let tz = plane_multiply(
                                plane_multiply(plane_coordinate(z), tool.cell_size()),
                                plane.add(2).read_unaligned(),
                            );
                            let height = plane_multiply(plane_add(tx + tz, plane.add(3).read_unaligned()), inverse);
                            let mut low = height_subtract(height, tool.tolerance()).to_f32();
                            let mut high = plane_add(height, tool.tolerance());
                            let terrain = f32::from_bits(word(cell, 0x58 + corner * 4));
                            if plane_less(plane_compare(F80::from_f32(terrain), low)) {
                                low = terrain;
                            } else if plane_less(plane_compare(high, terrain)) {
                                high = F80::from_f32(terrain);
                            }
                            let interval = cell.wrapping_add(0x68 + corner * 8);
                            if plane_less(plane_compare(F80::from_f32(f32::from_bits(word(interval, 0))), low)) {
                                put(interval, 0, low.to_bits());
                            }
                            if plane_less(plane_compare(high, f32::from_bits(word(interval, 4)))) {
                                put(interval, 4, high.to_f32().to_bits());
                            }
                            let low = f32::from_bits(word(interval, 0));
                            if plane_less(plane_compare(F80::from_f32(f32::from_bits(word(interval, 4))), low)) {
                                let gap = height_subtract(F80::from_f32(low), f32::from_bits(word(interval, 4)));
                                let status = plane_compare(gap, plane_interval_epsilon().read_unaligned());
                                if matches!(status & 0x4100, 0x0100 | 0x4000) {
                                    put(interval, 4, word(interval, 0));
                                }
                            }
                            let status = plane_compare(height, f32::from_bits(word(cell, 0x58 + corner * 4)));
                            let terrain = word(cell, 0x58 + corner * 4);
                            if status & 0x4100 == 0 {
                                put(interval, 0x24, height.to_f32().to_bits());
                                put(interval, 0x20, terrain);
                            } else {
                                put(interval, 0x20, height.to_f32().to_bits());
                                put(interval, 0x24, terrain);
                            }
                        }
                        let segments = tool.segments();
                        edx = offset;
                        eax = PlaneSegment::from_ptr(segments.wrapping_add(offset as usize)).last();
                        cell_index = cell_index.wrapping_add(1);
                        if cell_index > eax {
                            break;
                        }
                    }
                }
            }
            count = tool.count();
            index = index.wrapping_add(1);
            offset = offset.wrapping_add(12);
            if index >= count {
                break;
            }
        }
        save_result(frame, registers(eax, edx));
    }
}

#[cfg(test)]
extern "C" fn connection_prefix_targets() -> u64 {
    registers(
        connection_services_initialized() as u32,
        crate::abi::target(0x0065_471D) as u32,
    )
}

port! {
    /// Prepares the connection initializer's local frame, zero register and service flag comparison.
    /// Continues at the following entry with all incoming arguments and the prepared flags intact.
    #[port(address = 0x0065_4710, owner = "shared", name = "PrepareConnectionInitialization", naked)]
    pub unsafe extern "C" fn fc4p_00654710() {
        #[cfg(not(test))]
        core::arch::naked_asm!(
            "mov al, byte ptr [0x00b46838]", "sub esp, 0x54", "push ebx", "xor ebx, ebx", "cmp al, bl",
            "jmp dword ptr [2f]", "2: .long 0x0065471d",
        );
        #[cfg(test)]
        core::arch::naked_asm!(
            "push eax", "push ecx", "push edx", "call {targets}",
            "mov al, [eax]", "mov [esp + 8], al", "mov [esp - 4], edx",
            "pop edx", "pop ecx", "pop eax", "sub esp, 0x54", "push ebx", "xor ebx, ebx", "cmp al, bl",
            "jmp dword ptr [esp + 0x48]", targets = sym connection_prefix_targets,
        );
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00637b90::PORT,
    fc4p_00654710::PORT,
    fc4p_00658710::PORT,
    fc4p_00639170::PORT,
    fc4p_006392f0::PORT,
    fc4p_006444f0::PORT,
    fc4p_00644530::PORT,
    fc4p_00649820::PORT,
    fc4p_00657790::PORT,
    fc4p_00657860::PORT,
    fc4p_00657a30::PORT,
    fc4p_00668ab0::PORT,
    fc4p_0067e140::PORT,
    fc4p_0067e250::PORT,
    fc4p_0067f0d0::PORT,
    fc4p_006915e0::PORT,
    fc4p_00638990::PORT,
    fc4p_00657740::PORT,
    fc4p_00657af0::PORT,
    fc4p_00657be0::PORT,
    fc4p_00665db0::PORT,
    fc4p_0067c240::PORT,
    fc4p_0067cb20::PORT,
    fc4p_0067cd50::PORT,
    fc4p_0067e0b0::PORT,
    fc4p_0068b960::PORT,
    fc4p_006a9980::PORT,
    fc4p_006acb60::PORT,
    fc4p_006b5840::PORT,
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

    mod lane42 {
        use super::*;
        use crate::testing::{fake_data, Fake, GameFpu};

        #[derive(Default)]
        struct Fixture {
            planes: Vec<[f32; 4]>,
            plane_index: usize,
            cells: Vec<(u32, usize)>,
            output: usize,
            plane_mutations: Vec<(usize, usize, u32)>,
            cell_mutations: Vec<(usize, usize, u32)>,
            builder_registers: Vec<[u32; 2]>,
            cell_registers: Vec<[u32; 2]>,
        }
        thread_local! { static FIXTURE: RefCell<Fixture> = RefCell::new(Fixture::default()); }

        extern "fastcall" fn record_builder_registers(_this: *mut u8, frame: *const u32) {
            FIXTURE.with(|s| unsafe {
                s.borrow_mut()
                    .builder_registers
                    .push([frame.add(1).read(), frame.add(2).read()]);
            });
        }

        extern "fastcall" fn record_cell_registers(_this: *mut u8, frame: *const u32) {
            FIXTURE.with(|s| unsafe {
                s.borrow_mut()
                    .cell_registers
                    .push([frame.add(1).read(), frame.add(2).read()]);
            });
        }

        #[unsafe(naked)]
        unsafe extern "C" fn build_plane_entry() {
            core::arch::naked_asm!(
                "push edx", "push eax", "push ecx", "mov edx, esp", "call {record}",
                "pop ecx", "pop eax", "pop edx", "jmp {body}",
                record = sym record_builder_registers, body = sym build_plane,
            );
        }

        #[unsafe(naked)]
        unsafe extern "C" fn find_cell_entry() {
            core::arch::naked_asm!(
                "push edx", "push eax", "push ecx", "mov edx, esp", "call {record}",
                "pop ecx", "pop eax", "pop edx", "jmp {body}",
                record = sym record_cell_registers, body = sym find_cell,
            );
        }

        extern "thiscall" fn build_plane(
            this: *mut u8,
            output: *mut f32,
            first: i32,
            last: i32,
            near: u32,
            far: u32,
        ) -> u64 {
            log(0x0063_7780, this, &[first as u32, last as u32, near, far]);
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.output = output as usize;
                let plane = s.planes[s.plane_index];
                s.plane_index += 1;
                unsafe {
                    for (i, value) in plane.into_iter().enumerate() {
                        output.add(i).write_unaligned(value);
                    }
                    for &(p, offset, value) in &s.plane_mutations {
                        put(p as *mut u8, offset, value);
                    }
                }
            });
            registers(0xBEEF_1234, 0x1122_3344)
        }

        extern "thiscall" fn find_cell(this: *mut u8, key: u32) -> u64 {
            log(0x0063_3220, this, &[key]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    for &(p, offset, value) in &s.cell_mutations {
                        put(p as *mut u8, offset, value);
                    }
                }
                registers(s.cells.iter().find(|&&(k, _)| k == key).unwrap().1 as u32, 0x5566_7788)
            })
        }

        struct Harness {
            tool: Object,
            segments: Vec<[u32; 3]>,
            coordinates: Vec<[u32; 2]>,
            cells: Vec<[u32; 48]>,
            anchors: Vec<[u32; 4]>,
            _constants: Box<[u32; 13]>,
            _fakes: Vec<Fake>,
            _fpu: GameFpu,
        }

        impl Harness {
            fn new(segments: &[[u32; 3]], coordinates: &[[u32; 2]], planes: &[[f32; 4]]) -> Self {
                reset();
                FIXTURE.with(|s| {
                    *s.borrow_mut() = Fixture {
                        planes: planes.to_vec(),
                        ..Fixture::default()
                    }
                });
                let mut constants = Box::new([
                    0.0f32.to_bits(),
                    (-1.0f32).to_bits(),
                    4_294_967_296.0f32.to_bits(),
                    0.01f32.to_bits(),
                    0,
                    0,
                    1,
                    1,
                    0,
                    1,
                    1,
                    0,
                    0,
                ]);
                let mut fakes = vec![
                    fake(0x0063_7780, build_plane_entry as *const () as usize),
                    fake(0x0063_3220, find_cell_entry as *const () as usize),
                ];
                for (address, slot) in [
                    (0x00A8_1054, 0),
                    (0x00A8_FE78, 1),
                    (0x00A8_0AA8, 2),
                    (0x00AA_8A74, 3),
                    (0x00AA_8B30, 4),
                    (0x00AA_8B20, 8),
                ] {
                    fakes.push(fake_data(address, unsafe { constants.as_mut_ptr().add(slot).cast() }));
                }
                let mut h = Self {
                    tool: Object::new(0x2C0),
                    segments: segments.to_vec(),
                    coordinates: coordinates.to_vec(),
                    cells: vec![[0xA5A5_A5A5; 48]; coordinates.len()],
                    anchors: Vec::new(),
                    _constants: constants,
                    _fakes: fakes,
                    _fpu: GameFpu::new(),
                };
                unsafe {
                    let view = PlaneWindows::from_ptr(h.tool.ptr());
                    view.set_segments(h.segments.as_mut_ptr().cast());
                    view.set_coordinates(h.coordinates.as_mut_ptr().cast());
                    view.set_count(h.segments.len() as i32);
                    view.set_cell_size(16.0);
                    view.set_tolerance(35.0);
                }
                for (i, &[x, z]) in coordinates.iter().enumerate() {
                    h.cells[i][0] = x;
                    h.cells[i][1] = z;
                    for corner in 0..4 {
                        h.cells[i][0x58 / 4 + corner] = 300.0f32.to_bits();
                        h.cells[i][0x68 / 4 + corner * 2] = f32::NEG_INFINITY.to_bits();
                        h.cells[i][0x6C / 4 + corner * 2] = f32::INFINITY.to_bits();
                    }
                    FIXTURE.with(|s| {
                        s.borrow_mut()
                            .cells
                            .push((z.wrapping_shl(16).wrapping_add(x), h.cells[i].as_mut_ptr() as usize))
                    });
                }
                h
            }

            fn run(&mut self) -> (u32, u32) {
                let range = [
                    self.anchors.as_mut_ptr(),
                    self.anchors.as_mut_ptr().wrapping_add(self.anchors.len()),
                ];
                unsafe {
                    run(
                        fc4p_00637b90 as *const (),
                        Conv::Thiscall,
                        self.tool.ptr(),
                        &[range.as_ptr() as u32],
                        0xAABB_CCDD,
                        0x9876_5432,
                    )
                }
            }

            fn windows(&self, cell: usize) -> [[f32; 4]; 4] {
                core::array::from_fn(|k| {
                    [0x68, 0x6C, 0x88, 0x8C].map(|offset| f32::from_bits(self.cells[cell][offset / 4 + k * 2]))
                })
            }

            fn assert_calls(&mut self, expected: &[(u32, Vec<u32>)]) {
                assert_eq!(
                    calls(),
                    expected
                        .iter()
                        .map(|(id, args)| Call(*id, self.tool.ptr() as usize, args.clone()))
                        .collect::<Vec<_>>()
                );
            }
        }

        #[test]
        fn plane_windows_keep_the_live_terrain_fixture_and_touch_only_corner_ranges() {
            // The same four heights and plane used by network geometry's single-cell control.
            let mut h = Harness::new(&[[0, 0, 0]], &[[86, 3]], &[[0.0, 256.0, -25.332031, -73144.0]]);
            let terrain = [291.40268f32, 293.30893, 290.79285, 289.53262];
            for (i, value) in terrain.into_iter().enumerate() {
                h.cells[0][0x58 / 4 + i] = value.to_bits();
            }
            let mut expected = h.cells[0];
            let tool_before = bytes(h.tool.ptr(), 0x2C0);
            assert_eq!(h.run(), (0, 0));
            let heights = [290.4685f32, 292.05176, 292.05176, 290.4685];
            for k in 0..4 {
                let values = [
                    heights[k] - 35.0,
                    heights[k] + 35.0,
                    heights[k].min(terrain[k]),
                    heights[k].max(terrain[k]),
                ];
                for (offset, value) in [0x68, 0x6C, 0x88, 0x8C].into_iter().zip(values) {
                    expected[offset / 4 + k * 2] = value.to_bits();
                }
            }
            assert_eq!(h.cells[0], expected);
            assert_eq!(bytes(h.tool.ptr(), 0x2C0), tool_before);
            assert_eq!(h.segments, [[0, 0, 0]]);
            assert_eq!(h.coordinates, [[86, 3]]);
            h.assert_calls(&[(0x0063_7780, vec![0, 0, 0, 0]), (0x0063_3220, vec![0x0003_0056])]);
        }

        #[test]
        fn empty_counts_skipped_segments_and_empty_cell_ranges_preserve_their_result_words() {
            let mut h = Harness::new(&[[2, 1, 0]], &[], &[[0.0, 1.0, 0.0, -10.0]]);
            for count in [0, -1, i32::MIN] {
                unsafe {
                    PlaneWindows::from_ptr(h.tool.ptr()).set_count(count);
                }
                assert_eq!(h.run(), (0xAABB_CCDD, 0x9876_5432));
                assert!(calls().is_empty());
            }
            unsafe {
                PlaneWindows::from_ptr(h.tool.ptr()).set_count(1);
            }
            h.segments[0][2] = 0x8765_4321;
            assert_eq!(h.run(), (h.segments.as_ptr() as u32, 0x8765_4321));
            assert!(calls().is_empty());
            h.segments[0][2] = 0;
            assert_eq!(h.run(), (h.segments.as_ptr() as u32, h.segments.as_ptr() as u32));
            h.assert_calls(&[(0x0063_7780, vec![0, 0, 0, 0])]);
        }

        #[test]
        fn zero_plane_returns_after_the_builder_with_partial_eax_and_its_edx() {
            for b in [0.0, -0.0] {
                let mut h = Harness::new(&[[0, 0, 0]], &[[0, 0]], &[[1.0, b, 2.0, 3.0]]);
                let before = h.cells.clone();
                // The register probe keeps one x87 value live: TOP is seven at the comparison.
                assert_eq!(h.run(), (0xBEEF_7800, 0x1122_3344));
                assert_eq!(h.cells, before);
                h.assert_calls(&[(0x0063_7780, vec![0, 0, 0, 0])]);
            }
        }

        #[test]
        fn anchors_choose_nearest_strict_neighbors_and_reuse_the_plane_through_the_far_anchor() {
            let mut h = Harness::new(
                &[[0, 0, 1], [0, 0, 0], [1, 1, 0], [2, 2, 0], [3, 3, 0]],
                &[[1, 0], [2, 0], [3, 0], [4, 0]],
                &[[0.0, 1.0, 0.0, -300.0], [0.0, 1.0, 0.0, -310.0]],
            );
            h.anchors = vec![
                [4, 0, 40, 10],
                [3, 1, 30, 11],
                [2, 0, 20, 12],
                [2, 0, 99, 99],
                [i32::MIN as u32, i32::MAX as u32, 77, 88],
            ];
            assert_eq!(h.run(), (3, 48));
            h.assert_calls(&[
                (0x0063_7780, vec![0, 2, 10, 20]),
                (0x0063_3220, vec![1]),
                (0x0063_3220, vec![2]),
                (0x0063_7780, vec![1, 4, 11, 40]),
                (0x0063_3220, vec![3]),
                (0x0063_3220, vec![4]),
            ]);
            assert_eq!(h.windows(0), [[265.0, 335.0, 300.0, 300.0]; 4]);
            assert_eq!(h.windows(3), [[275.0, 345.0, 300.0, 310.0]; 4]);
        }

        #[test]
        fn clamping_intersects_existing_windows_and_repairs_only_small_ordered_inversions() {
            let mut h = Harness::new(&[[0, 0, 0]], &[[0, 0]], &[[0.0, 1.0, 0.0, -300.0]]);
            for (k, height) in [250.0f32, 350.0, 300.0, 300.0].into_iter().enumerate() {
                h.cells[0][0x58 / 4 + k] = height.to_bits();
            }
            h.cells[0][0x68 / 4 + 2 * 2] = 301.0f32.to_bits();
            h.cells[0][0x6C / 4 + 2 * 2] = 300.995f32.to_bits();
            h.cells[0][0x68 / 4 + 3 * 2] = 301.0f32.to_bits();
            h.cells[0][0x6C / 4 + 3 * 2] = 300.98f32.to_bits();
            h.run();
            assert_eq!(
                h.windows(0),
                [
                    [250.0, 335.0, 250.0, 300.0],
                    [265.0, 350.0, 300.0, 350.0],
                    [301.0, 301.0, 300.0, 300.0],
                    [301.0, 300.98, 300.0, 300.0],
                ]
            );
            for (gap, repair) in [
                (0.0f32, true),
                (0.01, true),
                (f32::from_bits(0.01f32.to_bits() + 1), false),
                (f32::NAN, false),
            ] {
                let mut h = Harness::new(&[[0, 0, 0]], &[[0, 0]], &[[0.0, 1.0, 0.0, 0.0]]);
                h.cells[0][0x58 / 4] = 0;
                h.cells[0][0x68 / 4] = gap.to_bits();
                h.cells[0][0x6C / 4] = 0;
                h.run();
                assert_eq!(h.cells[0][0x6C / 4], if repair { gap.to_bits() } else { 0 });
            }
        }

        #[test]
        fn coordinates_wrap_before_unsigned_x87_conversion_and_intermediates_keep_their_exponents() {
            let mut h = Harness::new(&[[0, 0, 0]], &[[u32::MAX, 0]], &[[1.0, -1.0, 0.0, 0.0]]);
            unsafe {
                PlaneWindows::from_ptr(h.tool.ptr()).set_cell_size(1.0);
            }
            h.run();
            assert_eq!(h.windows(0)[0][3], 4_294_967_296.0);
            assert_eq!(h.windows(0)[2][2..], [0.0, 300.0]);

            let mut h = Harness::new(&[[0, 0, 0]], &[[1, 0]], &[[f32::MAX, -1.0, 0.0, 0.0]]);
            unsafe {
                PlaneWindows::from_ptr(h.tool.ptr()).set_cell_size(2.0);
            }
            // The product exceeds single precision's exponent range, then returns to it.
            FIXTURE.with(|s| s.borrow_mut().planes[0][1] = -f32::MAX);
            h.run();
            assert!(h.windows(0)[0][2].is_finite());
            assert_eq!(h.windows(0)[0][2].to_bits(), 2.0f32.to_bits() - 1);
        }

        #[test]
        fn unordered_planes_and_terrain_follow_the_comparison_branches_and_keep_payloads() {
            for b in [f32::from_bits(0x7FC1_2345), f32::from_bits(0x7F81_2345)] {
                let mut h = Harness::new(&[[0, 0, 0]], &[[0, 0]], &[[0.0, b, 0.0, 0.0]]);
                let before = h.cells[0];
                h.run();
                for k in 0..4 {
                    assert_eq!(h.cells[0][0x68 / 4 + k * 2], before[0x68 / 4 + k * 2]);
                    assert_eq!(h.cells[0][0x6C / 4 + k * 2], before[0x6C / 4 + k * 2]);
                    assert_eq!(h.cells[0][0x88 / 4 + k * 2], 0x7FC1_2345);
                    assert_eq!(h.cells[0][0x8C / 4 + k * 2], 300.0f32.to_bits());
                }
                h.assert_calls(&[(0x0063_7780, vec![0, 0, 0, 0]), (0x0063_3220, vec![0])]);
            }
            let mut h = Harness::new(&[[0, 0, 0]], &[[0, 0]], &[[0.0, 1.0, 0.0, -300.0]]);
            h.cells[0][0x58 / 4] = 0x7F81_2345;
            h.run();
            assert_eq!(h.windows(0)[0][..3], [265.0, 335.0, 300.0]);
            assert_eq!(h.cells[0][0x8C / 4], 0x7F81_2345);
        }

        #[test]
        fn plane_and_cell_calls_can_replace_live_storage_and_shorten_the_scan() {
            let mut h = Harness::new(&[[9, 8, 0], [9, 8, 1]], &[[1, 2], [3, 4]], &[[0.0, 1.0, 0.0, -300.0]]);
            let mut replacement = [[0u32, 1, 0], [0, 0, 0]];
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.plane_mutations
                    .push((h.tool.ptr() as usize, 0x54, replacement.as_mut_ptr() as u32));
                s.cell_mutations = vec![
                    (replacement.as_mut_ptr() as usize, 4, 0),
                    (h.tool.ptr() as usize, 0x78, 1),
                ];
            });
            assert_eq!(h.run(), (0, 0));
            h.assert_calls(&[(0x0063_7780, vec![0, 1, 0, 0]), (0x0063_3220, vec![0x0002_0001])]);
            assert_eq!(h.cells[1][0x88 / 4], 0xA5A5_A5A5);
        }

        #[unsafe(naked)]
        unsafe extern "C" fn plane_stack_probe() {
            core::arch::naked_asm!("mov [ecx + 0x2b4], esp", "jmp {entry}", entry = sym fc4p_00637b90);
        }

        #[test]
        fn inclusive_cell_ranges_pass_the_original_registers_and_plane_output_address() {
            let mut h = Harness::new(&[[0, 1, 0], [2, 1, 7]], &[[5, 6], [7, 8]], &[[0.0, 1.0, 0.0, -300.0]]);
            let range = [h.anchors.as_mut_ptr(); 2];
            let result = unsafe {
                run(
                    plane_stack_probe as *const (),
                    Conv::Thiscall,
                    h.tool.ptr(),
                    &[range.as_ptr() as u32],
                    0,
                    0,
                )
            };
            assert_eq!(result, (h.segments.as_ptr() as u32, 7));
            FIXTURE.with(|s| {
                let s = s.borrow();
                assert_eq!(s.builder_registers, [[1, 0]]);
                assert_eq!(s.cell_registers, [[0x0006_0005, 5], [0x0008_0007, 7]]);
                assert_eq!(s.output as u32, unsafe { word(h.tool.ptr(), 0x2B4) }.wrapping_sub(16));
            });
            h.assert_calls(&[
                (0x0063_7780, vec![0, 1, 0, 0]),
                (0x0063_3220, vec![0x0006_0005]),
                (0x0063_3220, vec![0x0008_0007]),
            ]);
        }

        #[unsafe(naked)]
        unsafe extern "C" fn prefix_continuation() {
            core::arch::naked_asm!(
                "pushfd",
                "pop dword ptr [ecx + 12]",
                "mov [ecx], eax",
                "mov [ecx + 4], edx",
                "mov [ecx + 8], ebx",
                "lea eax, [esp + 0x58]",
                "mov [ecx + 16], eax",
                "mov eax, [esp + 0x5c]",
                "mov [ecx + 20], eax",
                "mov eax, [esp + 0x60]",
                "mov [ecx + 24], eax",
                "mov eax, [ecx]",
                "pop ebx",
                "add esp, 0x54",
                "ret 8",
            );
        }

        #[unsafe(naked)]
        unsafe extern "C" fn prefix_probe() {
            core::arch::naked_asm!("mov [ecx + 28], esp", "jmp {entry}", entry = sym fc4p_00654710);
        }

        #[test]
        fn connection_prefix_prepares_the_full_frame_and_flags_without_consuming_arguments() {
            let mut flag = 0u8;
            let _data = fake_data(0x00B4_6838, &mut flag);
            let _next = fake(0x0065_471D, prefix_continuation as *const () as usize);
            for byte in [0u8, 1, 0x7F, 0x80, 0xFF] {
                flag = byte;
                let mut recorded = [0u32; 8];
                let out = unsafe {
                    run(
                        prefix_probe as *const (),
                        Conv::Thiscall,
                        recorded.as_mut_ptr().cast(),
                        &[0x1234_ABCD, 0x9876_EF01],
                        0xAABB_CCDD,
                        0x1122_3344,
                    )
                };
                assert_eq!(recorded[0], 0xAABB_CC00 | u32::from(byte));
                assert_eq!(recorded[1..3], [0x1122_3344, 0]);
                let expected_flags = (u32::from(byte == 0) * 0x40)
                    | (u32::from(byte & 0x80 != 0) * 0x80)
                    | (u32::from(byte.count_ones().is_multiple_of(2)) * 4);
                assert_eq!(recorded[3] & 0x8D5, expected_flags);
                assert_eq!(recorded[4], recorded[7]);
                assert_eq!(recorded[5..7], [0x1234_ABCD, 0x9876_EF01]);
                assert_eq!(out, (recorded[0], 0x1122_3344));
                assert_eq!(flag, byte);
            }
        }
    }

    mod lane30 {
        use super::*;
        use crate::testing::{fake_data, GameFpu};
        use crate::x87::F80;

        #[derive(Default)]
        struct Fixture {
            words: [u32; 12],
            pointers: [usize; 8],
        }
        thread_local! { static FIXTURE: RefCell<Fixture> = RefCell::new(Fixture::default()); }
        fn reset() {
            super::reset();
            FIXTURE.with(|s| *s.borrow_mut() = Fixture::default());
        }
        extern "thiscall" fn solved(this: *mut u8, vertex: u32, output: *mut u32) -> u64 {
            log(0x0065_EB00, this, &[vertex, unsafe { output.read_unaligned() }]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    output.write_unaligned(s.words[1]);
                }
                registers(s.words[0], 0x5566_7788)
            })
        }
        extern "thiscall" fn terrain_height(this: *mut u8, vertex: u32) -> f32 {
            log(0x30, this, &[vertex]);
            12.5
        }

        #[test]
        fn height_getters_choose_the_solution_or_unrounded_terrain_and_apply_only_the_fallback_offset() {
            reset();
            let _fpu = GameFpu::new();
            let _lookup = fake(0x0065_EB00, solved as *const () as usize);
            let mut vt = [0usize; 13];
            vt[12] = terrain_height as *const () as usize;
            let mut terrain = [vt.as_ptr() as usize];
            let t = terrain.as_mut_ptr().cast::<u8>();
            let mut object = [0xA5u8; 0x380];
            let p = object.as_mut_ptr();
            unsafe {
                put(p, 0x44, t as u32);
                put(p, 0x36C, 2.25f32.to_bits());
            }
            let before = object;
            for (entry, offset) in [(fc4p_006444f0 as *const (), 0.0), (fc4p_00657790 as *const (), 2.25)] {
                for result in [0xABCD_0000, 0xABCD_0080] {
                    FIXTURE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.words[0] = result;
                        s.words[1] = 7.75f32.to_bits();
                    });
                    let args = [0x8000_1234];
                    let mut call = RawCall::new(Conv::Thiscall, entry as usize, p as u32, &args);
                    call.take_st0 = 1;
                    unsafe {
                        crate::abi::call_raw(&mut call);
                    }
                    let expected = if result as u8 == 0 { 12.5 - offset } else { 7.75 };
                    assert_eq!(call.st0, F80::from_f32(expected));
                    let mut expected_calls = vec![Call(
                        0x0065_EB00,
                        p.wrapping_add(0x124) as usize,
                        vec![args[0], args[0]],
                    )];
                    if result as u8 == 0 {
                        expected_calls.push(Call(0x30, t as usize, vec![args[0]]));
                    }
                    assert_eq!(calls(), expected_calls);
                    if result as u8 != 0 {
                        assert_eq!((call.eax, call.edx), (result, 0x5566_7788));
                    }
                    assert_eq!(object, before);
                }
            }
        }

        static EXTENDED_HEIGHT: [u8; 10] = [0, 0, 0, 0, 2, 0, 0, 0x80, 0xFF, 0x3F];
        #[unsafe(naked)]
        unsafe extern "C" fn extended_terrain_height() {
            core::arch::naked_asm!(
                "fld tbyte ptr [{value}]", "mov eax, 0x11223344", "mov edx, 0x55667788", "ret 4",
                value = sym EXTENDED_HEIGHT,
            );
        }
        #[test]
        fn height_getters_keep_extended_st0_bits_and_the_terrain_integer_registers() {
            reset();
            let _fpu = GameFpu::new();
            let _lookup = fake(0x0065_EB00, solved as *const () as usize);
            let mut vt = [0usize; 13];
            vt[12] = extended_terrain_height as *const () as usize;
            let mut terrain = [vt.as_ptr() as usize];
            let mut object = [0u8; 0x380];
            let p = object.as_mut_ptr();
            unsafe {
                put(p, 0x44, terrain.as_mut_ptr() as u32);
                put(p, 0x36C, 1f32.to_bits());
            }
            for (entry, expected) in [
                (fc4p_006444f0 as *const (), F80::from_bytes(EXTENDED_HEIGHT)),
                (fc4p_00657790 as *const (), F80::from_f32(f32::from_bits(0x3080_0000))),
            ] {
                let args = [7];
                let mut call = RawCall::new(Conv::Thiscall, entry as usize, p as u32, &args);
                call.take_st0 = 1;
                unsafe {
                    crate::abi::call_raw(&mut call);
                }
                assert_eq!(call.st0, expected);
                assert_eq!((call.eax, call.edx), (0x1122_3344, 0x5566_7788));
                assert_eq!(crate::x87::control_word(), crate::x87::GAME_CONTROL_WORD);
            }
            calls();
        }

        #[test]
        fn direction_scaling_handles_axes_signs_zero_infinities_and_nan() {
            let _fpu = GameFpu::new();
            let mut zero = 0f32;
            let _zero = fake_data(0x00A8_1054, (&mut zero as *mut f32).cast());
            for (dx, dz, x, z) in [
                (4.0, 2.0, 1.0f32, 0.5f32),
                (-2.0, 4.0, -0.5, 1.0),
                (3.0, -3.0, 1.0, -1.0),
                (0.0, 0.0, 0.0, 1.0),
                (-0.0, -0.0, 0.0, 1.0),
                (-4.0, -2.0, -1.0, -0.5),
                (f32::INFINITY, 1.0, 1.0, 0.0),
                (1.0, f32::INFINITY, 0.0, 1.0),
            ] {
                let mut object = [0xA5u8; 32];
                let p = object.as_mut_ptr();
                let result = unsafe { fc4p_00649820(p, 0x1122_3344, 0xAABB_CCDD, dx, dz, 0x300) };
                let mut expected = [0xA5u8; 32];
                for (offset, word) in [
                    (0, 0x300u32),
                    (4, 0),
                    (8, 0x1122_3344),
                    (12, 0xAABB_CCDD),
                    (16, x.to_bits()),
                    (20, z.to_bits()),
                ] {
                    expected[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                }
                assert_eq!(object, expected);
                assert_eq!(result, registers(0xAABB_CCDD, 0x1122_3344));
            }
            let mut object = [0u8; 24];
            unsafe {
                fc4p_00649820(object.as_mut_ptr(), 0, 0, f32::NAN, 1.0, 0);
            }
            assert!(f32::from_bits(unsafe { word(object.as_ptr(), 16) }).is_nan());
            assert_eq!(unsafe { word(object.as_ptr(), 20) }, 1f32.to_bits());
        }

        extern "thiscall" fn edge(this: *mut u8, a: u32, b: u32) -> u64 {
            log(0x0065_FDB0, this, &[a, b]);
            registers(0x1234_5678, 0x8765_4321)
        }
        extern "thiscall" fn edge_kind(this: *mut u8) -> u64 {
            log(0x5C, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().words[0], 0x1020_3040))
        }
        extern "thiscall" fn dimensions(this: *mut u8, a: *mut u32, b: *mut u32) -> u64 {
            log(0xEC, this, &[unsafe { b.read_unaligned() }]);
            unsafe {
                a.write_unaligned(7);
                b.write_unaligned(11);
            }
            registers(0xAABB_CCDD, 0x1020_3040)
        }
        #[test]
        fn cell_edges_skip_special_kinds_and_link_only_three_valid_neighbor_pairs() {
            reset();
            let _cell = fake(0x0063_3220, cell as *const () as usize);
            let _edge = fake(0x0065_FDB0, edge as *const () as usize);
            let mut object = [0u32; 0x130 / 4];
            let p = object.as_mut_ptr().cast::<u8>();
            let mut cell_memory = [0u8; 0xBC];
            let c = cell_memory.as_mut_ptr();
            STATE.with(|s| s.borrow_mut().cell = c as usize);
            let mut kinds = [0x900u32, 0, 0];
            unsafe {
                put(p, 0x98, kinds.as_mut_ptr() as u32);
                for (i, value) in [5u32, 8, u32::MAX, 20].into_iter().enumerate() {
                    put(c, 0xA8 + i * 4, value);
                }
            }
            cell_memory[0x57] = 1;
            for kind in [0, 0x200, 0x300, 0x800, 0xA00, 0x1400, 0x4B00, 0x14E00, 0x900] {
                kinds[0] = kind;
                let result = unsafe { run(fc4p_00657860 as *const (), Conv::Thiscall, p, &[99], 0, 0) };
                let mut expected = vec![Call(0x0063_3220, p as usize, vec![99])];
                if kind == 0x900 {
                    expected.push(Call(0x0065_FDB0, p.wrapping_add(0x124) as usize, vec![5, 8]));
                    assert_eq!(result, (3, 0x8765_4321));
                } else {
                    assert_eq!(result, (0, kinds.as_ptr() as u32));
                }
                assert_eq!(calls(), expected);
            }
            cell_memory[0x56] = 1;
            unsafe {
                kinds.as_mut_ptr().write(0x900);
            }
            unsafe {
                run(fc4p_00657860 as *const (), Conv::Thiscall, p, &[99], 0, 0);
            }
            assert_eq!(
                calls(),
                [
                    Call(0x0063_3220, p as usize, vec![99]),
                    Call(0x0065_FDB0, p.wrapping_add(0x124) as usize, vec![100_005, 100_008]),
                    Call(0x0065_FDB0, p.wrapping_add(0x124) as usize, vec![100_008, 99_999]),
                    Call(0x0065_FDB0, p.wrapping_add(0x124) as usize, vec![99_999, 100_020])
                ]
            );
            let mut vt = [0usize; 0xF0 / 4];
            vt[0x5C / 4] = edge_kind as *const () as usize;
            vt[0xEC / 4] = dimensions as *const () as usize;
            let mut occupant = [vt.as_ptr() as usize];
            unsafe {
                put(c, 0x0C, occupant.as_mut_ptr() as u32);
            }
            cell_memory[0x57] = 0;
            FIXTURE.with(|s| s.borrow_mut().words[0] = 0x300);
            assert_eq!(
                unsafe { run(fc4p_00657860 as *const (), Conv::Thiscall, p, &[99], 0, 0) },
                (0xAABB_CCDD, 0x1020_3040)
            );
            assert_eq!(
                calls().iter().map(|c| c.0).collect::<Vec<_>>(),
                [0x0063_3220, 0x5C, 0xEC]
            );
            for (offset, value) in [(0x53, 1), (0x53, 0)] {
                cell_memory[offset] = value;
                unsafe {
                    put(c, 12, 0);
                    run(fc4p_00657860 as *const (), Conv::Thiscall, p, &[99], 0, 0);
                }
                assert_eq!(calls().len(), 1);
            }
            STATE.with(|s| s.borrow_mut().cell = 0);
            assert_eq!(
                unsafe { run(fc4p_00657860 as *const (), Conv::Thiscall, p, &[99], 0, 0) },
                (0, 0x0033_4455)
            );
            calls();
        }

        extern "thiscall" fn flags_query(this: *mut u8, iid: u32, output: *mut *mut u8) -> u64 {
            log(0, this, &[iid, unsafe { output.read() } as u32]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    output.write(s.pointers[0] as *mut u8);
                }
                registers(s.words[0], 0x1122_3344)
            })
        }
        extern "thiscall" fn remove_flag(this: *mut u8, flag: u32) -> u64 {
            log(0x2C, this, &[flag]);
            registers(0x1234_5678, 0x90AB_CDEF)
        }
        extern "thiscall" fn add_flag(this: *mut u8, flag: u32) -> u64 {
            log(0x28, this, &[flag]);
            registers(0x2233_4455, 0x6677_8899)
        }
        extern "thiscall" fn statuses(this: *mut u8, a: *mut u8, b: *mut u8, c: *mut u8) -> u64 {
            log(0x170, this, &[]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    a.write(s.words[1] as u8);
                    b.write(s.words[2] as u8);
                    c.write(s.words[3] as u8);
                }
            });
            registers(0xAABB_CCDD, 0x1122_3344)
        }
        #[test]
        fn occupant_flags_cover_every_status_combination_and_release_even_a_failed_query_output() {
            reset();
            let mut vt = [0usize; 0x174 / 4];
            vt[0] = flags_query as *const () as usize;
            vt[8 / 4] = release as *const () as usize;
            vt[0x28 / 4] = add_flag as *const () as usize;
            vt[0x2C / 4] = remove_flag as *const () as usize;
            vt[0x170 / 4] = statuses as *const () as usize;
            let mut occupant = [vt.as_ptr() as usize];
            let p = occupant.as_mut_ptr().cast::<u8>();
            for bits in 0..8u32 {
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.pointers[0] = p as usize;
                    s.words[0] = 0x1234_0080;
                    for i in 0..3 {
                        s.words[i + 1] = ((bits >> i) & 1) * 0x80;
                    }
                });
                let result = unsafe {
                    run(
                        fc4p_00657a30 as *const (),
                        Conv::Stdcall,
                        core::ptr::null_mut(),
                        &[p as u32, 0xDEAD_BEEF],
                        0,
                        0,
                    )
                };
                assert_eq!(result, (0xA1B2_C3D4, 0x0077_8899));
                let mut expected = vec![Call(0, p as usize, vec![0x4A10_AFC2, 0])];
                for flag in [0x0100_0000, 0x0200_0000, 0x0400_0000] {
                    expected.push(Call(0x2C, p as usize, vec![flag]));
                }
                expected.push(Call(0x170, p as usize, vec![]));
                for (flag, enabled) in [
                    (0x0100_0000, bits & 1 != 0),
                    (0x0200_0000, bits & 2 == 0),
                    (0x0400_0000, bits & 4 == 0),
                ] {
                    if enabled {
                        expected.push(Call(0x28, p as usize, vec![flag]));
                    }
                }
                expected.push(Call(8, p as usize, vec![]));
                assert_eq!(calls(), expected);
            }
            for output in [0, p as usize] {
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.pointers[0] = output;
                    s.words[0] = 0xAABB_0000;
                });
                let result = unsafe { run(fc4p_00657a30 as *const (), Conv::Stdcall, p, &[p as u32, 0], 0, 0) };
                assert_eq!(
                    result,
                    if output == 0 {
                        (0xAABB_0000, 0x1122_3344)
                    } else {
                        (0xA1B2_C3D4, 0x0077_8899)
                    }
                );
                assert_eq!(calls().len(), if output == 0 { 1 } else { 2 });
            }
        }

        #[allow(clippy::too_many_arguments)]
        extern "C" fn clear_ranges(a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32, flag: u32) -> u64 {
            log(0x008E_BB10, core::ptr::null_mut(), &[a, b, c, d, e, f, g, h, flag]);
            registers(0x1122_3344, 0x5566_7788)
        }
        extern "thiscall" fn clear_range(this: *mut u8, first: u32, last: u32) -> u64 {
            log(0x0066_7D40, this, &[first, last]);
            FIXTURE.with(|s| {
                if s.borrow().words[0] != 0 {
                    unsafe {
                        put(this, 0x20, 0);
                    }
                }
            });
            registers(0x2233_4455, 0x6677_8899)
        }
        extern "thiscall" fn pool_free(this: *mut u8, allocation: *mut u8) -> u64 {
            log(4, this, &[allocation as u32]);
            registers(0x3344_5566, 0x7788_99AA)
        }
        extern "C" fn large_free(allocation: *mut u8) -> u64 {
            log(0x009F_015B, allocation, &[]);
            registers(0x4455_6677, 0x8899_AABB)
        }
        #[test]
        fn range_destruction_copies_both_descriptors_and_selects_every_allocator_tag() {
            reset();
            let _clear = fake(0x008E_BB10, clear_ranges as *const () as usize);
            let _range = fake(0x0066_7D40, clear_range as *const () as usize);
            let _large = fake(0x009F_015B, large_free as *const () as usize);
            let vt = [0usize, pool_free as *const () as usize];
            let mut pools = [[vt.as_ptr() as usize]; 9];
            let mut pool_ptrs = pools.each_mut().map(|p| p.as_mut_ptr().cast::<u8>());
            let _pools = fake_data(0x00B6_2674, pool_ptrs.as_mut_ptr().cast());
            let mut allocation = [0u8; 16];
            let a = allocation.as_mut_ptr();
            let mut object = [1u32, 2, 3, 4, 5, 6, 7, u32::MAX, 0];
            let p = object.as_mut_ptr().cast::<u8>();
            for tag in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 255] {
                allocation[3] = tag;
                object[8] = a.wrapping_add(4) as u32;
                let result = unsafe { run(fc4p_00668ab0 as *const (), Conv::Thiscall, p, &[], 0, 0) };
                assert_eq!(
                    result,
                    if tag < 9 {
                        (0x3344_5566, 0x7788_99AA)
                    } else {
                        (0x4455_6677, 0x8899_AABB)
                    }
                );
                let trace = calls();
                assert_eq!(trace[0], Call(0x008E_BB10, 0, vec![1, 2, 3, 4, 5, 6, 7, u32::MAX, 0]));
                assert_eq!(trace[1], Call(0x0066_7D40, p as usize, vec![4, 3]));
                assert_eq!(
                    trace[2],
                    if tag < 9 {
                        Call(4, pool_ptrs[tag as usize] as usize, vec![a as u32])
                    } else {
                        Call(0x009F_015B, a as usize, vec![])
                    }
                );
            }
            FIXTURE.with(|s| s.borrow_mut().words[0] = 1);
            assert_eq!(
                unsafe { run(fc4p_00668ab0 as *const (), Conv::Thiscall, p, &[], 0, 0) },
                (0x2233_4455, 0x6677_8899)
            );
            assert_eq!(calls().len(), 2);
            assert_eq!(
                unsafe { run(fc4p_00668ab0 as *const (), Conv::Thiscall, p, &[], 0, 0) },
                (0, 0x5566_7788)
            );
            assert_eq!(calls().len(), 1);
        }

        extern "C" fn allocate_vector(size: u32) -> *mut u8 {
            log(0x0090_CF54, core::ptr::null_mut(), &[size]);
            FIXTURE.with(|s| s.borrow().pointers[0] as *mut u8)
        }
        extern "C" fn vector_free(allocation: *mut u8) -> u64 {
            log(0x0090_CF63, allocation, &[]);
            registers(0xDEAD_BEEF, 0xFEED_FACE)
        }
        extern "C" fn copy_records(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 {
            log(
                0x006E_9150,
                first,
                &[last as u32, out as u32, unsafe { cookie.read_unaligned() }],
            );
            let mut first = first;
            let mut out = out;
            unsafe {
                while first != last {
                    for offset in (0..32).step_by(4) {
                        put(out, offset, word(first, offset));
                    }
                    first = first.add(32);
                    out = out.add(32);
                }
                cookie.write_unaligned(0xABCD_EF80);
            }
            registers(out as u32, 0x1122_3344)
        }
        #[test]
        fn vector_growth_covers_empty_single_multiple_insertions_suffix_modes_and_aliased_values() {
            reset();
            let _allocate = fake(0x0090_CF54, allocate_vector as *const () as usize);
            let _free = fake(0x0090_CF63, vector_free as *const () as usize);
            let _copy = fake(0x006E_9150, copy_records as *const () as usize);
            for (length, at_index, count, skip) in [
                (0usize, 0usize, 0usize, 1u32),
                (0, 0, 1, 1),
                (3, 0, 1, 0),
                (3, 1, 2, 0),
                (3, 3, 5, 0),
                (3, 2, 0, 0),
                (3, 1, 2, 0x100),
            ] {
                let mut source: Vec<u32> = (0..24).map(|n| n + 100).collect();
                let a = source.as_mut_ptr().cast::<u8>();
                let mut destination = [0xCCCC_CCCCu32; 96];
                let b = destination.as_mut_ptr().cast::<u8>();
                FIXTURE.with(|s| s.borrow_mut().pointers[0] = b as usize);
                let mut object = [a as u32, a.wrapping_add(length * 32) as u32, a.wrapping_add(96) as u32];
                let p = object.as_mut_ptr().cast::<u8>();
                let result = unsafe {
                    run(
                        fc4p_006915e0 as *const (),
                        Conv::Thiscall,
                        p,
                        &[
                            a.wrapping_add(at_index * 32) as u32,
                            a as u32,
                            0xDEAD_BEEF,
                            count as u32,
                            skip,
                        ],
                        0,
                        0,
                    )
                };
                let capacity = length + length.max(count);
                let allocation = if capacity == 0 { 0 } else { b as u32 };
                let suffix = if skip as u8 == 0 { length - at_index } else { 0 };
                assert_eq!(result.0, allocation);
                assert_eq!(
                    object,
                    [
                        allocation,
                        allocation.wrapping_add(((at_index + count + suffix) * 32) as u32),
                        allocation.wrapping_add((capacity * 32) as u32)
                    ]
                );
                let mut expected = source[..at_index * 8].to_vec();
                for _ in 0..count {
                    expected.extend_from_slice(&source[..8]);
                }
                if suffix != 0 {
                    expected.extend_from_slice(&source[at_index * 8..length * 8]);
                }
                assert_eq!(&destination[..expected.len()], expected);
                let mut trace = Vec::new();
                if capacity != 0 {
                    trace.push(Call(0x0090_CF54, 0, vec![(capacity * 32) as u32]));
                }
                if skip as u8 == 0 {
                    trace.push(Call(
                        0x006E_9150,
                        a.wrapping_add(at_index * 32) as usize,
                        vec![
                            a.wrapping_add(length * 32) as u32,
                            b.wrapping_add((at_index + count) * 32) as u32,
                            skip,
                        ],
                    ));
                }
                trace.push(Call(0x0090_CF63, a as usize, vec![]));
                assert_eq!(calls(), trace);
            }
        }

        extern "thiscall" fn rewind(this: *mut u8) -> u64 {
            log(0x0065_EF70, this, &[]);
            FIXTURE.with(|s| s.borrow_mut().words[10] = 0);
            0
        }
        extern "thiscall" fn next_height(this: *mut u8, vertex: *mut u32, height: *mut u32) -> u64 {
            log(0x0065_EB50, this, &[]);
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                let i = s.words[10] as usize;
                if i >= s.words[9] as usize {
                    return registers(0xABCD_0000, 0x1122_3344);
                }
                unsafe {
                    vertex.write_unaligned(s.words[i * 2]);
                    height.write_unaligned(s.words[i * 2 + 1]);
                }
                s.words[10] += 1;
                registers(0xABCD_0080, 0x1122_3344)
            })
        }
        extern "thiscall" fn flat_height(this: *mut u8, vertex: u32) -> f32 {
            log(0x30, this, &[vertex]);
            300.0
        }
        extern "thiscall" fn set_height(this: *mut u8, vertex: u32, height: u32) -> u64 {
            log(0xC8, this, &[vertex, height]);
            registers(0x1234_5678, 0x8765_4321)
        }
        extern "thiscall" fn begin_grading(this: *mut u8) -> u64 {
            log(0x15C, this, &[]);
            0
        }
        extern "thiscall" fn end_grading(this: *mut u8) -> u64 {
            log(0x160, this, &[]);
            registers(0xAABB_CCDD, 0x1122_3344)
        }
        extern "thiscall" fn redisplay(this: *mut u8, a: u32, b: u32, rect: *const u32, kind: u32) -> u64 {
            let r = unsafe { core::slice::from_raw_parts(rect, 4) };
            log(0x110, this, &[a, b, r[0], r[1], r[2], r[3], kind]);
            0
        }
        extern "thiscall" fn width(this: *mut u8) -> u64 {
            log(0x174, this, &[]);
            registers(64, 0x1122_3344)
        }
        extern "thiscall" fn depth(this: *mut u8) -> u64 {
            log(0x178, this, &[]);
            registers(64, 0x5566_7788)
        }
        extern "thiscall" fn grow_vertices(
            this: *mut u8,
            at: *mut u8,
            value: *const u32,
            cookie: *mut u32,
            count: u32,
            mode: u32,
        ) -> u64 {
            log(
                0x0051_CA60,
                this,
                &[
                    at as u32,
                    unsafe { value.read_unaligned() },
                    unsafe { cookie.read_unaligned() },
                    count,
                    mode,
                ],
            );
            FIXTURE.with(|s| {
                let allocation = s.borrow().pointers[0] as *mut u8;
                unsafe {
                    put(allocation, 0, value.read_unaligned());
                    put(this, 0, allocation as u32);
                    put(this, 4, allocation.wrapping_add(4) as u32);
                    put(this, 8, allocation.wrapping_add(32) as u32);
                }
            });
            registers(0x1234_5678, 0x8765_4321)
        }
        #[test]
        fn grading_keeps_threshold_bounds_solution_order_and_the_z_divisor() {
            reset();
            let _fpu = GameFpu::new();
            let _rewind = fake(0x0065_EF70, rewind as *const () as usize);
            let _next = fake(0x0065_EB50, next_height as *const () as usize);
            let _grow = fake(0x0051_CA60, grow_vertices as *const () as usize);
            let mut threshold = 0.001f32;
            let _threshold = fake_data(0x00A8_67A4, (&mut threshold as *mut f32).cast());
            let mut terrain_vt = [0usize; 0x17C / 4];
            terrain_vt[0x30 / 4] = flat_height as *const () as usize;
            terrain_vt[0xC8 / 4] = set_height as *const () as usize;
            terrain_vt[0x110 / 4] = redisplay as *const () as usize;
            terrain_vt[0x15C / 4] = begin_grading as *const () as usize;
            terrain_vt[0x160 / 4] = end_grading as *const () as usize;
            terrain_vt[0x174 / 4] = width as *const () as usize;
            terrain_vt[0x178 / 4] = depth as *const () as usize;
            let mut terrain = [terrain_vt.as_ptr() as usize];
            let t = terrain.as_mut_ptr().cast::<u8>();
            let mut city = t;
            let _city = fake_data(0x00B4_3CEC, (&mut city as *mut *mut u8).cast());
            let mut object = [0xA5u8; 0x260];
            let p = object.as_mut_ptr();
            unsafe {
                put(p, 0x44, t as u32);
                put(p, 0x248, 65);
                put(p, 0x24C, 65);
            }
            let original = object;
            let mut vertices = [0xCCCC_CCCCu32; 8];
            let v = vertices.as_mut_ptr().cast::<u8>();
            for (entries, expected, rect) in [
                (
                    vec![(660u32, 300.0005f32), (1330, 300.002)],
                    vec![1330u32],
                    Some([29, 19, 31, 21]),
                ),
                (vec![(660u32, 300.0005f32)], vec![], None),
                (
                    vec![(0u32, 310.0f32), (4224, 310.0)],
                    vec![0u32, 4224],
                    Some([0, 0, 64, 64]),
                ),
                (vec![(1u32, f32::NAN)], vec![], None),
                (vec![], vec![], None),
            ] {
                let mut changed = [v as u32, v.wrapping_add(12) as u32, v.wrapping_add(32) as u32];
                let c = changed.as_mut_ptr().cast::<u8>();
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.words[9] = entries.len() as u32;
                    for (i, (vertex, height)) in entries.iter().enumerate() {
                        s.words[i * 2] = *vertex;
                        s.words[i * 2 + 1] = height.to_bits();
                    }
                });
                let result = unsafe { run(fc4p_00644530 as *const (), Conv::Thiscall, p, &[c as u32], 0, 0) };
                assert_eq!(changed[1], v.wrapping_add(expected.len() * 4) as u32);
                assert_eq!(&vertices[..expected.len()], expected);
                let trace = calls();
                assert_eq!(trace[0], Call(0x0065_EF70, p.wrapping_add(0x188) as usize, vec![]));
                assert_eq!(trace.iter().filter(|c| c.0 == 0x0065_EB50).count(), entries.len() + 1);
                let expected_writes: Vec<_> = entries
                    .iter()
                    .filter(|(v, _)| expected.contains(v))
                    .map(|(v, h)| Call(0xC8, t as usize, vec![*v, h.to_bits()]))
                    .collect();
                assert_eq!(
                    trace.iter().filter(|c| c.0 == 0xC8).collect::<Vec<_>>(),
                    expected_writes.iter().collect::<Vec<_>>()
                );
                if let Some(r) = rect {
                    assert_eq!(
                        &trace[trace.len() - 5..],
                        [
                            Call(0x174, t as usize, vec![]),
                            Call(0x178, t as usize, vec![]),
                            Call(0x15C, t as usize, vec![]),
                            Call(0x110, t as usize, vec![1, 1, r[0], r[1], r[2], r[3], 4]),
                            Call(0x160, t as usize, vec![])
                        ]
                    );
                    assert_eq!(result, (0xAABB_CCDD, 0x1122_3344));
                } else {
                    assert_eq!(result, (64, 0));
                }
                assert_eq!(object, original);
            }
            // A non-square grid uses +0x24c for Z, including after vector growth.
            unsafe {
                put(p, 0x24C, 33);
            }
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.words[9] = 1;
                s.words[0] = 100;
                s.words[1] = 301f32.to_bits();
                s.pointers[0] = v as usize;
            });
            let mut changed = [0u32; 3];
            unsafe {
                run(
                    fc4p_00644530 as *const (),
                    Conv::Thiscall,
                    p,
                    &[changed.as_mut_ptr() as u32],
                    0,
                    0,
                );
            }
            let trace = calls();
            assert!(trace.contains(&Call(0x0051_CA60, changed.as_ptr() as usize, vec![0, 100, 100, 1, 1])));
            assert!(trace.contains(&Call(0x110, t as usize, vec![1, 1, 34, 2, 36, 4, 4])));
        }

        extern "thiscall" fn copy_cells(this: *mut u8, output: *mut u32) -> u64 {
            log(0x0064_7E60, this, unsafe { core::slice::from_raw_parts(output, 3) });
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    output.write(s.pointers[0] as u32);
                    output
                        .add(1)
                        .write((s.pointers[0] as u32).wrapping_add(s.words[9] * 0xBC));
                    output
                        .add(2)
                        .write((s.pointers[0] as u32).wrapping_add(s.words[9] * 0xBC));
                }
            });
            registers(0x1234_5678, 0x8765_4321)
        }
        extern "thiscall" fn underground_service(this: *mut u8) -> u64 {
            log(0x144, this, &[]);
            registers(0x1020, 0x1122_3344)
        }
        extern "C" fn find_underground(x: u32, z: u32, output: *mut *mut u8, service: u32) -> u64 {
            log(
                0x0065_84D0,
                service as *mut u8,
                &[x, z, unsafe { output.read() } as u32],
            );
            FIXTURE.with(|s| unsafe {
                output.write(s.borrow().pointers[1] as *mut u8);
            });
            registers(0xABCD_0000, 0x2233_4455)
        }
        extern "thiscall" fn underground_flag(this: *mut u8, flag: u32) -> u64 {
            log(0x20, this, &[flag]);
            FIXTURE.with(|s| registers(s.borrow().words[8], 0x3344_5566))
        }
        extern "thiscall" fn old_corners(this: *mut u8, output: *mut u32) -> u64 {
            log(0x88, this, &[]);
            unsafe {
                for i in 0..4 {
                    output.add(i).write(300f32.to_bits());
                }
            }
            registers(0x4455_6677, 0x5566_7788)
        }
        extern "thiscall" fn desired_corner(this: *mut u8, vertex: u32) -> f32 {
            log(0x124, this, &[vertex]);
            FIXTURE.with(|s| f32::from_bits(s.borrow().words[(vertex - 100) as usize]))
        }
        extern "thiscall" fn set_corners(this: *mut u8, values: *const u32) -> u64 {
            log(0x150, this, unsafe { core::slice::from_raw_parts(values, 4) });
            registers(0x6677_8899, 0x7788_99AA)
        }
        extern "thiscall" fn dirty_occupant(this: *mut u8, value: u32) -> u64 {
            log(0x158, this, &[value]);
            registers(0x8899_AABB, 0x99AA_BBCC)
        }
        #[test]
        fn underground_refresh_checks_copied_flags_hash_chains_and_all_four_unrounded_heights_before_releasing() {
            reset();
            let _fpu = GameFpu::new();
            let _copy = fake(0x0064_7E60, copy_cells as *const () as usize);
            let _find = fake(0x0065_84D0, find_underground as *const () as usize);
            let _free = fake(0x0090_CF63, vector_free as *const () as usize);
            let mut threshold = 0.001f32;
            let _threshold = fake_data(0x00A8_67A4, (&mut threshold as *mut f32).cast());
            let mut tool_vt = [0usize; 0x148 / 4];
            tool_vt[0x124 / 4] = desired_corner as *const () as usize;
            tool_vt[0x144 / 4] = underground_service as *const () as usize;
            let mut found_vt = [0usize; 0x15C / 4];
            found_vt[8 / 4] = release as *const () as usize;
            found_vt[0x20 / 4] = underground_flag as *const () as usize;
            found_vt[0x88 / 4] = old_corners as *const () as usize;
            found_vt[0x150 / 4] = set_corners as *const () as usize;
            found_vt[0x158 / 4] = dirty_occupant as *const () as usize;
            let mut found = [found_vt.as_ptr() as usize];
            let f = found.as_mut_ptr().cast::<u8>();
            let mut retained = [found_vt.as_ptr() as usize];
            let r = retained.as_mut_ptr().cast::<u8>();
            let mut record = [0u8; 0xBC];
            let c = record.as_mut_ptr();
            unsafe {
                put(c, 12, r as u32);
                for i in 0..4 {
                    put(c, 0xA8 + i * 4, (100 + i) as u32);
                }
            }
            let mut match_node = [0u32, 0];
            let mut collision = [match_node.as_mut_ptr() as u32, 1];
            let mut buckets = [collision.as_mut_ptr() as u32];
            let mut object = [0u32; 0xB0 / 4];
            let p = object.as_mut_ptr().cast::<u8>();
            unsafe {
                put(p, 0, tool_vt.as_ptr() as u32);
                put(p, 0xA8, buckets.as_mut_ptr() as u32);
                put(p, 0xAC, buckets.as_mut_ptr().add(1) as u32);
            }
            for (skip, present, found_occupant, flag, height, changed) in [
                (true, true, true, 0u32, 301f32, false),
                (false, false, true, 0, 301.0, false),
                (false, true, false, 0, 301.0, false),
                (false, true, true, 0xABCD_0080, 301.0, false),
                (false, true, true, 0xABCD_0000, 300.0005, false),
                (false, true, true, 0, 300.002, true),
                (false, true, true, 0, f32::NAN, false),
            ] {
                record[0x54] = u8::from(skip);
                unsafe {
                    collision
                        .as_mut_ptr()
                        .write(if present { match_node.as_mut_ptr() as u32 } else { 0 });
                }
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.pointers[0] = c as usize;
                    s.pointers[1] = if found_occupant { f as usize } else { 0 };
                    s.words[9] = 1;
                    s.words[8] = flag;
                    s.words[..4].fill(300f32.to_bits());
                    s.words[2] = height.to_bits();
                });
                assert_eq!(
                    unsafe { run(fc4p_00658710 as *const (), Conv::Thiscall, p, &[], 0, 0) },
                    (0xDEAD_BEEF, 0xFEED_FACE)
                );
                let trace = calls();
                let mut ids = vec![0x0064_7E60];
                if !skip && present {
                    ids.extend([0x144, 0x0065_84D0]);
                    if found_occupant {
                        ids.push(0x20);
                        if flag as u8 == 0 {
                            ids.extend([0x88, 0x124, 0x124, 0x124, 0x124]);
                            if changed {
                                ids.extend([0x150, 0x158]);
                            }
                        }
                        ids.push(8);
                    }
                }
                ids.extend([8, 0x0090_CF63]);
                assert_eq!(trace.iter().map(|c| c.0).collect::<Vec<_>>(), ids);
                assert_eq!(
                    trace[0],
                    Call(0x0064_7E60, p.wrapping_add(0x1C) as usize, vec![0, 0, 0])
                );
                assert_eq!(trace[trace.len() - 2], Call(8, r as usize, vec![]));
                if changed {
                    assert!(trace.contains(&Call(
                        0x150,
                        f as usize,
                        vec![300f32.to_bits(), 300f32.to_bits(), height.to_bits(), 300f32.to_bits()]
                    )));
                }
            }
            // No records and no allocation retain the copy helper's register result.
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.pointers[0] = 0;
                s.words[9] = 0;
            });
            assert_eq!(
                unsafe { run(fc4p_00658710 as *const (), Conv::Thiscall, p, &[], 0, 0) },
                (0x1234_5678, 0x8765_4321)
            );
            assert_eq!(
                calls(),
                [Call(0x0064_7E60, p.wrapping_add(0x1C) as usize, vec![0, 0, 0])]
            );
        }

        extern "C" fn allocate_string(size: u32) -> *mut u8 {
            log(0x0090_CF54, core::ptr::null_mut(), &[size]);
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                let i = s.words[10] as usize;
                s.words[10] += 1;
                s.pointers[i] as *mut u8
            })
        }
        extern "thiscall" fn read_pair(this: *mut u8, key: *mut u32, value: *mut u32) -> u64 {
            unsafe {
                for string in [key, value] {
                    let data = string.add(1).read();
                    assert_eq!(
                        core::slice::from_raw_parts(string, 5),
                        &[0x00A8_0810, data, data, data + 8, 0]
                    );
                    assert_eq!((data as *const u8).read(), 0);
                    if FIXTURE.with(|s| s.borrow().words[5] != 0) {
                        string.add(1).write(0);
                    }
                }
                log(0x0067_E0B0, this, &[key.add(1).read(), value.add(1).read()]);
            }
            FIXTURE.with(|s| registers(s.borrow().words[0], 0x1122_3344))
        }
        extern "C" fn compare_key(a: *const u8, b: *const u8) -> u64 {
            log(0x00A0_4487, a.cast_mut(), &[b as u32]);
            FIXTURE.with(|s| registers(s.borrow().words[1], 0x2233_4455))
        }
        extern "C" fn integer_entry(text: *const u8) -> u64 {
            log(0x0090_FE93, text.cast_mut(), &[]);
            registers(0xFEDC_BA98, 0x3344_5566)
        }
        extern "thiscall" fn boolean_entry_body(string: *mut u8) -> u64 {
            log(0x0067_DFD0, core::ptr::null_mut(), &[unsafe { word(string, 4) }]);
            FIXTURE.with(|s| registers(s.borrow().words[2], 0x4455_6677))
        }
        #[unsafe(naked)]
        unsafe extern "C" fn boolean_entry() {
            core::arch::naked_asm!("mov ecx, eax", "jmp {body}", body = sym boolean_entry_body);
        }
        extern "thiscall" fn value_entry_body(string: *mut u8, output: *mut u32) -> u64 {
            log(
                0x0067_E780,
                core::ptr::null_mut(),
                &[unsafe { word(string, 4) }, output as u32, unsafe {
                    output.read_unaligned()
                }],
            );
            unsafe {
                output.write_unaligned(0xDEAD_BEEF);
            }
            FIXTURE.with(|s| registers(s.borrow().words[2], 0x4455_6677))
        }
        #[unsafe(naked)]
        unsafe extern "C" fn value_entry() {
            core::arch::naked_asm!("push dword ptr [esp + 4]", "mov ecx, eax", "call {body}", "ret", body = sym value_entry_body);
        }
        #[test]
        fn entry_readers_keep_call_order_ownership_failed_outputs_and_boolean_bytes() {
            reset();
            let _allocate = fake(0x0090_CF54, allocate_string as *const () as usize);
            let _free = fake(0x0090_CF63, vector_free as *const () as usize);
            let _pair = fake(0x0067_E0B0, read_pair as *const () as usize);
            let _compare = fake(0x00A0_4487, compare_key as *const () as usize);
            let _integer = fake(0x0090_FE93, integer_entry as *const () as usize);
            let _boolean = fake(0x0067_DFD0, boolean_entry as *const () as usize);
            let _value = fake(0x0067_E780, value_entry as *const () as usize);
            let mut key_buffer = [0xCCu8; 8];
            let mut value_buffer = [0xCCu8; 8];
            let k = key_buffer.as_mut_ptr();
            let v = value_buffer.as_mut_ptr();
            let parser = 0x1020 as *mut u8;
            let mut expected_string = [0u32, 0x3040];
            for lane in 0..3 {
                for (read, compare, interpreted, null_strings) in [
                    (0u32, 0, 0, false),
                    (0xABCD_0080, 1, 0, false),
                    (1, u32::MAX, 0, false),
                    (1, 0, 0, false),
                    (1, 0, 0xAABB_0080, false),
                    (0, 0, 0, true),
                ] {
                    FIXTURE.with(|s| {
                        let mut s = s.borrow_mut();
                        s.words[0] = read;
                        s.words[1] = compare;
                        s.words[2] = interpreted;
                        s.words[5] = u32::from(null_strings);
                        s.words[10] = 0;
                        s.pointers[0] = k as usize;
                        s.pointers[1] = v as usize;
                    });
                    let mut output = 0xA5A5_A5A5u32;
                    let out = &mut output as *mut u32;
                    let result = unsafe {
                        match lane {
                            0 => fc4p_0067e140(parser, expected_string.as_mut_ptr().cast()),
                            1 => fc4p_0067e250(parser, expected_string.as_mut_ptr().cast(), out),
                            _ => fc4p_0067f0d0(parser, out, out.cast()),
                        }
                    };
                    let parsed = read as u8 != 0 && (lane == 2 || compare == 0);
                    let answer = if !parsed {
                        0
                    } else if lane == 0 {
                        interpreted as u8
                    } else if lane == 1 || interpreted as u8 != 0 {
                        1
                    } else {
                        0
                    };
                    assert_eq!(result as u8, answer);
                    assert_eq!(
                        output,
                        if parsed && lane == 1 {
                            0xFEDC_BA98
                        } else if parsed && lane == 2 {
                            0xDEAD_BEEF
                        } else {
                            0xA5A5_A5A5
                        }
                    );
                    let mut trace = vec![
                        Call(0x0090_CF54, 0, vec![8]),
                        Call(0x0090_CF54, 0, vec![8]),
                        Call(
                            0x0067_E0B0,
                            parser as usize,
                            if null_strings {
                                vec![0, 0]
                            } else {
                                vec![k as u32, v as u32]
                            },
                        ),
                    ];
                    if read as u8 != 0 && lane != 2 {
                        trace.push(Call(0x00A0_4487, k as usize, vec![0x3040]));
                    }
                    if parsed {
                        match lane {
                            0 => trace.push(Call(0x0067_DFD0, 0, vec![v as u32])),
                            1 => trace.push(Call(0x0090_FE93, v as usize, vec![])),
                            _ => {
                                trace.push(Call(0x0090_FE93, k as usize, vec![]));
                                trace.push(Call(0x0067_E780, 0, vec![v as u32, out as u32, 0xFEDC_BA98]));
                            }
                        }
                    }
                    if !null_strings {
                        trace.push(Call(0x0090_CF63, v as usize, vec![]));
                        trace.push(Call(0x0090_CF63, k as usize, vec![]));
                    }
                    assert_eq!(calls(), trace);
                    assert_eq!(&key_buffer[1..], &[0xCC; 7]);
                    assert_eq!(&value_buffer[1..], &[0xCC; 7]);
                }
            }
        }
    }

    mod occupant_lists30 {
        use super::*;
        use crate::testing::fake_data;

        #[derive(Default)]
        struct Fixture {
            pointers: [usize; 6],
            results: [u32; 6],
            visits: u32,
            reset_allocator: bool,
        }
        thread_local! { static FIXTURE: RefCell<Fixture> = RefCell::new(Fixture::default()); }
        extern "C" fn framework() -> u64 {
            log(0x0087_93EC, core::ptr::null_mut(), &[]);
            FIXTURE.with(|s| registers(s.borrow().pointers[2] as u32, 0x1122_3344))
        }
        extern "thiscall" fn query_allocator(this: *mut u8, class: u32, iid: u32, out: *mut *mut u8) -> u64 {
            log(0x14, this, &[class, iid, unsafe { out.read() } as u32]);
            FIXTURE.with(|s| unsafe {
                out.write(s.borrow().pointers[1] as *mut u8);
            });
            registers(0xAABB_0080, 0x2233_4455)
        }
        extern "thiscall" fn release_allocator(temporary: *mut *mut u8) -> u64 {
            let allocator = unsafe { temporary.read_unaligned() };
            log(0x0041_1870, allocator, &[]);
            assert_eq!(unsafe { occupant_list_allocator().read_unaligned() }, allocator);
            registers(0xAABB_CCDD, 0x3344_5566)
        }
        extern "thiscall" fn allocate(this: *mut u8, size: u32) -> *mut u8 {
            log(0x0C, this, &[size]);
            FIXTURE.with(|s| s.borrow().pointers[0] as *mut u8)
        }
        extern "thiscall" fn free_list(this: *mut u8, node: u32) -> u64 {
            log(0x10, this, &[node]);
            registers(0xAABB_CCDD, 0x4455_6677)
        }
        extern "thiscall" fn clear(list: *mut u32) -> u64 {
            let sentinel = unsafe { list.read_unaligned() };
            log(0x0041_3E60, core::ptr::null_mut(), &[sentinel]);
            unsafe {
                put(sentinel as *mut u8, 0, sentinel);
                put(sentinel as *mut u8, 4, sentinel);
            }
            FIXTURE.with(|s| {
                if s.borrow().reset_allocator {
                    unsafe {
                        occupant_list_allocator().write_unaligned(core::ptr::null_mut());
                    }
                }
            });
            registers(sentinel, 0x5566_7788)
        }
        extern "thiscall" fn blocking(this: *mut u8, x: u32, z: u32, context: u32) -> u64 {
            log(0x10C, this, &[x, z, context]);
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.visits += 1;
                registers(
                    if s.visits == s.results[0] {
                        0xABCD_0080
                    } else {
                        0xABCD_0000
                    },
                    0x1122_3344,
                )
            })
        }
        fn allocator_table() -> [usize; 6] {
            [
                0,
                0,
                0,
                allocate as *const () as usize,
                free_list as *const () as usize,
                query_allocator as *const () as usize,
            ]
        }

        #[test]
        fn network_search_allocates_even_when_skipped_and_checks_segments_before_cached_extras() {
            super::reset();
            let _framework = fake(0x0087_93EC, framework as *const () as usize);
            let _release = fake(0x0041_1870, release_allocator as *const () as usize);
            let _clear = fake(0x0041_3E60, clear as *const () as usize);
            let vt = allocator_table();
            let mut allocator = [vt.as_ptr() as usize];
            let a = allocator.as_mut_ptr().cast::<u8>();
            let mut allocator_global = a;
            let _data = fake_data(0x00B2_07E4, (&mut allocator_global as *mut *mut u8).cast());
            let mut sentinel = [0xA5A5_A5A5u32; 3];
            let h = sentinel.as_mut_ptr().cast::<u8>();
            let mut owner_vt = [0usize; 0x110 / 4];
            owner_vt[0x10C / 4] = blocking as *const () as usize;
            let mut object = [0u32; 0x100 / 4];
            let p = object.as_mut_ptr().cast::<u8>();
            let mut rows = [[0u32, 0, 3], [1, 2, 0], [3, 2, 0]];
            let mut coordinates = [[11u32, 21], [12, 22], [13, 23]];
            let mut extras = [[31u32, 41, 0, 51, 0], [32, 42, 0, 52, 0]];
            unsafe {
                put(p, 0, owner_vt.as_ptr() as u32);
                put(p, 0x54, rows.as_mut_ptr() as u32);
                put(p, 0x60, coordinates.as_mut_ptr() as u32);
                put(p, 0xF0, extras.as_mut_ptr() as u32);
                put(p, 0xF4, extras.as_mut_ptr().add(2) as u32);
            }
            let original = object;
            for (count_initial, skip, success, expected_count, arguments) in [
                (3i32, 0u32, 1u32, 1i32, vec![vec![12, 22, 99]]),
                (3, 0, 3, 0, vec![vec![12, 22, 99], vec![13, 23, 99], vec![31, 41, 51]]),
                (3, 0x80, 1, 0, vec![vec![31, 41, 51]]),
                (-1, 0, 0, -1, vec![vec![31, 41, 51], vec![32, 42, 52]]),
                (
                    3,
                    0x100,
                    0,
                    3,
                    vec![vec![12, 22, 99], vec![13, 23, 99], vec![31, 41, 51], vec![32, 42, 52]],
                ),
            ] {
                FIXTURE.with(|s| {
                    *s.borrow_mut() = Fixture {
                        pointers: [h as usize, a as usize, a as usize, 0, 0, 0],
                        results: [success, 0, 0, 0, 0, 0],
                        ..Fixture::default()
                    }
                });
                let mut count = count_initial;
                assert_eq!(
                    unsafe {
                        run(
                            fc4p_00639170 as *const (),
                            Conv::Thiscall,
                            p,
                            &[&mut count as *mut i32 as u32, 99, skip],
                            0,
                            0,
                        )
                    },
                    (0xAABB_CCDD, 0x4455_6677)
                );
                assert_eq!(count, expected_count);
                let mut expected = vec![Call(0x0C, a as usize, vec![12])];
                expected.extend(arguments.into_iter().map(|args| Call(0x10C, p as usize, args)));
                expected.extend([
                    Call(0x0041_3E60, 0, vec![h as u32]),
                    Call(0x10, a as usize, vec![h as u32]),
                ]);
                assert_eq!(calls(), expected);
                assert_eq!(sentinel, [h as u32, h as u32, 0xA5A5_A5A5]);
                assert_eq!(object, original);
            }
            // Reacquire the allocator both at entry and after the list clear changes it.
            allocator_global = core::ptr::null_mut();
            assert!(allocator_global.is_null());
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.reset_allocator = true;
                s.visits = 0;
            });
            let mut count = 0i32;
            unsafe {
                run(
                    fc4p_00639170 as *const (),
                    Conv::Thiscall,
                    p,
                    &[&mut count as *mut i32 as u32, 0, 1],
                    0,
                    0,
                );
            }
            let trace = calls();
            assert_eq!(trace.iter().filter(|c| c.0 == 0x0087_93EC).count(), 2);
            assert_eq!(trace.iter().filter(|c| c.0 == 0x0041_1870).count(), 2);
            assert_eq!(allocator_global, a);
        }

        extern "thiscall" fn occupants(
            this: *mut u8,
            list: *mut u32,
            first: *const u32,
            last: *const u32,
            mode: u32,
            mask: u32,
        ) -> u64 {
            assert_eq!(first, last);
            log(
                0xA0,
                this,
                &[unsafe { first.read() }, unsafe { first.add(1).read() }, mode, mask],
            );
            FIXTURE.with(|s| {
                let s = s.borrow();
                let sentinel = unsafe { list.read() } as *mut u8;
                unsafe {
                    put(
                        sentinel,
                        0,
                        if s.pointers[3] == 0 {
                            sentinel as u32
                        } else {
                            s.pointers[3] as u32
                        },
                    );
                }
            });
            0
        }
        extern "thiscall" fn occupant_kind(this: *mut u8) -> u32 {
            log(0x1C, this, &[]);
            unsafe { word(this, 4) }
        }
        extern "thiscall" fn query_network(this: *mut u8, iid: u32, output: *mut *mut u8) -> u64 {
            log(0, this, &[iid, unsafe { output.read() } as u32]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    output.write(s.pointers[4] as *mut u8);
                }
                registers(s.results[1], 0x1122_3344)
            })
        }
        extern "thiscall" fn network_active(this: *mut u8) -> u64 {
            log(0x44, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().results[2], 0x2233_4455))
        }
        extern "thiscall" fn network_flag(this: *mut u8, flag: u32) -> u64 {
            log(0x20, this, &[flag]);
            FIXTURE.with(|s| registers(s.borrow().results[3], 0x3344_5566))
        }
        extern "thiscall" fn compatible(this: *mut u8, context: u32) -> u64 {
            log(0x58, this, &[context]);
            FIXTURE.with(|s| registers(s.borrow().results[4], 0x4455_6677))
        }
        #[test]
        fn point_occupants_cover_type_interface_active_flag_and_compatibility_branches_without_early_exit() {
            super::reset();
            let _clear = fake(0x0041_3E60, clear as *const () as usize);
            let allocator_vt = allocator_table();
            let mut allocator = [allocator_vt.as_ptr() as usize];
            let a = allocator.as_mut_ptr().cast::<u8>();
            let mut global = a;
            let _data = fake_data(0x00B2_07E4, (&mut global as *mut *mut u8).cast());
            let mut sentinel = [0u32; 3];
            let h = sentinel.as_mut_ptr().cast::<u8>();
            let mut occupant_vt = [0usize; 0x5C / 4];
            occupant_vt[0] = query_network as *const () as usize;
            occupant_vt[8 / 4] = release as *const () as usize;
            occupant_vt[0x1C / 4] = occupant_kind as *const () as usize;
            occupant_vt[0x20 / 4] = network_flag as *const () as usize;
            occupant_vt[0x44 / 4] = network_active as *const () as usize;
            occupant_vt[0x58 / 4] = compatible as *const () as usize;
            let mut occupant = [occupant_vt.as_ptr() as usize, 0];
            let o = occupant.as_mut_ptr().cast::<u8>();
            let mut network = [occupant_vt.as_ptr() as usize, 0];
            let n = network.as_mut_ptr().cast::<u8>();
            let mut node = [h as u32, h as u32, o as u32];
            let mut service_vt = [0usize; 0xA4 / 4];
            service_vt[0xA0 / 4] = occupants as *const () as usize;
            let mut service = [service_vt.as_ptr() as usize];
            let service_ptr = service.as_mut_ptr().cast::<u8>();
            let mut object = [0u32; 0x4C / 4];
            object[0x48 / 4] = service_ptr as u32;
            let p = object.as_mut_ptr().cast::<u8>();
            for (kind, query, active, flag, fits, blocked, suffix) in [
                (0x2781_28A0u32, 0, 0, 0, 0, false, vec![]),
                (0x6A11_BD2B, 0, 0, 0, 0, true, vec![]),
                (0x1234_5678, 0, 0, 0, 0, false, vec![]),
                (0xC772_BF98, 0xABCD_0000, 1, 1, 1, false, vec![0, 8]),
                (0xC772_BF98, 0xABCD_0080, 0xABCD_0000, 1, 1, false, vec![0, 0x44, 8]),
                (
                    0xC772_BF98,
                    1,
                    0xABCD_0080,
                    0xABCD_0000,
                    1,
                    true,
                    vec![0, 0x44, 0x20, 8],
                ),
                (0xC772_BF98, 1, 1, 1, 0xABCD_0000, true, vec![0, 0x44, 0x20, 0x58, 8]),
                (0xC772_BF98, 1, 1, 1, 0xABCD_0080, false, vec![0, 0x44, 0x20, 0x58, 8]),
            ] {
                unsafe {
                    put(o, 4, kind);
                }
                FIXTURE.with(|s| {
                    *s.borrow_mut() = Fixture {
                        pointers: [
                            h as usize,
                            a as usize,
                            a as usize,
                            node.as_mut_ptr() as usize,
                            n as usize,
                            0,
                        ],
                        results: [0, query, active, flag, fits, 0],
                        ..Fixture::default()
                    }
                });
                let result = unsafe {
                    run(
                        fc4p_006392f0 as *const (),
                        Conv::Thiscall,
                        p,
                        &[u32::MAX, 0x8000_0000, 99],
                        0,
                        0,
                    )
                };
                assert_eq!(result, (0xAABB_CC00 | u32::from(blocked), 0x4455_6677));
                let trace = calls();
                let mut ids = vec![0x0C, 0xA0, 0x1C];
                ids.extend(suffix);
                ids.extend([8, 0x0041_3E60, 0x10]);
                assert_eq!(trace.iter().map(|c| c.0).collect::<Vec<_>>(), ids);
                assert_eq!(
                    trace[1],
                    Call(0xA0, service_ptr as usize, vec![u32::MAX, 0x8000_0000, 0, u32::MAX])
                );
                if flag as u8 != 0 && query as u8 != 0 && active as u8 != 0 {
                    assert!(trace.contains(&Call(0x58, n as usize, vec![99])));
                }
            }
            // A positive result still visits and releases later occupants.
            let mut second = [h as u32, h as u32, o as u32];
            unsafe {
                node.as_mut_ptr().write(second.as_mut_ptr() as u32);
            }
            unsafe {
                put(o, 4, 0x6A11_BD2B);
            }
            unsafe {
                run(fc4p_006392f0 as *const (), Conv::Thiscall, p, &[0, 0, 0], 0, 0);
            }
            let trace = calls();
            assert_eq!(trace.iter().filter(|c| c.0 == 0x1C).count(), 2);
            assert_eq!(trace.iter().filter(|c| c.0 == 8).count(), 2);
        }
    }

    mod lane18 {
        use super::*;

        #[derive(Default)]
        struct Fixture {
            words: [u32; 8],
            pointers: [usize; 8],
            snapshots: Vec<Vec<u8>>,
        }
        thread_local! { static FIXTURE: RefCell<Fixture> = RefCell::new(Fixture::default()); }

        fn reset() {
            super::reset();
            FIXTURE.with(|s| *s.borrow_mut() = Fixture::default());
        }

        extern "thiscall" fn gate(this: *mut u8) -> u64 {
            log(0xFC, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().words[0], 0x1234_5678))
        }
        extern "thiscall" fn land(this: *mut u8, x: u32, z: u32) -> u64 {
            log(0x88, this, &[x, z]);
            FIXTURE.with(|s| registers(s.borrow().words[1], 0xAABB_CCDD))
        }
        extern "thiscall" fn cell(this: *mut u8, key: u32) -> u64 {
            log(0x0063_3220, this, &[key]);
            FIXTURE.with(|s| registers(s.borrow().pointers[0] as u32, 0x8765_4321))
        }

        #[test]
        fn segment_check_preserves_full_returns_and_reports_each_rejection_kind() {
            reset();
            let _lookup = fake(0x0063_3220, cell as *const () as usize);
            let mut table = [0usize; 0x100 / 4];
            table[0xFC / 4] = gate as *const () as usize;
            let mut terrain_table = [0usize; 0x8C / 4];
            terrain_table[0x88 / 4] = land as *const () as usize;
            let mut terrain = [terrain_table.as_ptr() as usize];
            let t = terrain.as_mut_ptr().cast::<u8>();
            let mut object = [0u32; 0x100 / 4];
            let p = object.as_mut_ptr().cast::<u8>();
            let mut rows = [[0u32, 0, 7], [0, 0, 0]];
            let mut coordinates = [0xFFFF_FFFFu32, 0xFFFF_8000];
            let mut extra = [0u32; 5];
            let e = extra.as_mut_ptr().cast::<u8>();
            let mut found = [0u8; 0x60];
            let f = found.as_mut_ptr();
            object[0] = table.as_ptr() as u32;
            object[0x44 / 4] = t as u32;
            object[0x54 / 4] = rows.as_mut_ptr() as u32;
            object[0x60 / 4] = coordinates.as_mut_ptr() as u32;
            object[0xF0 / 4] = e as u32;
            object[0xF4 / 4] = e as u32;
            for (gate_value, land_value, present, active, with_extra, success) in [
                (0xDEAD_BE00, 1, false, 0, false, true),
                (1, 0xDEAD_BE00, false, 0, false, true),
                (1, 1, false, 0, false, false),
                (1, 1, true, 0, false, false),
                (1, 1, true, 0x80, false, true),
                (1, 0, false, 0, true, false),
                (1, 1, true, 0x80, true, false),
            ] {
                let mut count = 2i32;
                found[0x56] = active;
                object[0xF4 / 4] = if with_extra {
                    e.wrapping_add(20) as u32
                } else {
                    e as u32
                };
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.words[0] = gate_value;
                    s.words[1] = land_value;
                    s.pointers[0] = if present { f as usize } else { 0 };
                });
                let before = object;
                let regs = unsafe {
                    run(
                        fc4p_00638990 as *const (),
                        Conv::Thiscall,
                        p,
                        &[(&mut count as *mut i32) as u32],
                        0,
                        0,
                    )
                };
                assert_eq!(regs.0 as u8 != 0, success);
                assert_eq!(
                    count,
                    if success {
                        2
                    } else if with_extra {
                        0
                    } else {
                        1
                    }
                );
                assert_eq!(object, before);
                let mut expected = vec![Call(0xFC, p as usize, vec![])];
                if gate_value as u8 != 0 {
                    expected.push(Call(0x88, t as usize, coordinates.to_vec()));
                    if land_value as u8 != 0 {
                        expected.push(Call(0x0063_3220, p as usize, vec![0x7FFF_FFFF]));
                    }
                    if with_extra {
                        expected.push(Call(0x88, t as usize, vec![0, 0]));
                        if land_value as u8 != 0 {
                            expected.push(Call(0x0063_3220, p as usize, vec![0]));
                        }
                    }
                }
                assert_eq!(calls(), expected);
                if gate_value as u8 == 0 {
                    assert_eq!(regs, (0xDEAD_BE01, 0x1234_5678));
                } else if !success && !with_extra {
                    assert_eq!(regs, (0, 0x8765_4321));
                } else if success {
                    assert_eq!(
                        regs,
                        (((e as u32) & !0xFF) | 1, (&mut count as *mut i32) as u32)
                    );
                }
            }
            // A negative count skips the segments; extra inactive cells are accepted.
            let mut count = -1i32;
            unsafe {
                f.add(0x56).write(0);
            }
            FIXTURE.with(|s| {
                s.borrow_mut().words[1] = 1;
            });
            let result = unsafe {
                run(
                    fc4p_00638990 as *const (),
                    Conv::Thiscall,
                    p,
                    &[(&mut count as *mut i32) as u32],
                    0,
                    0,
                )
            };
            assert_eq!(result.0 as u8, 1);
            assert_eq!(count, -1);
            assert_eq!(
                calls(),
                [
                    Call(0xFC, p as usize, vec![]),
                    Call(0x88, t as usize, vec![0, 0]),
                    Call(0x0063_3220, p as usize, vec![0])
                ]
            );
        }

        extern "thiscall" fn fetch(this: *mut u8, a: u32, out: *mut u32, b: u32, c: u32) -> u64 {
            log(0x148, this, &[a, unsafe { out.read_unaligned() }, b, c]);
            unsafe {
                out.write_unaligned(0xFEDC_BA98);
            }
            registers(0x1111_2222, 0x3333_4444)
        }

        #[test]
        fn optional_fetch_keeps_null_path_registers_and_passes_the_original_output_word() {
            reset();
            unsafe {
                assert_eq!(
                    run(
                        fc4p_00657740 as *const (),
                        Conv::Thiscall,
                        core::ptr::null_mut(),
                        &[1, 0, 2, 3],
                        0x1234_5678,
                        0x90AB_CDEF
                    ),
                    (0x1234_5678, 0x90AB_CDEF)
                );
            }
            assert!(calls().is_empty());
            let mut vt = [0usize; 0x14C / 4];
            vt[0x148 / 4] = fetch as *const () as usize;
            let mut object = [vt.as_ptr() as usize];
            let p = object.as_mut_ptr().cast();
            let mut output = [0xA5A5_A5A5u32; 3];
            let out = (&mut output[1] as *mut u32) as u32;
            let result = unsafe {
                run(
                    fc4p_00657740 as *const (),
                    Conv::Thiscall,
                    p,
                    &[u32::MAX, out, 0x8000_0000, 9],
                    0,
                    0,
                )
            };
            assert_eq!(result, (0xFEDC_BA98, 0x3333_4444));
            assert_eq!(output, [0xA5A5_A5A5, 0xFEDC_BA98, 0xA5A5_A5A5]);
            assert_eq!(
                calls(),
                [Call(0x148, p as usize, vec![u32::MAX, out, 0x8000_0000, 9])]
            );
        }

        #[unsafe(naked)]
        unsafe extern "C" fn preserve_four_words() {
            core::arch::naked_asm!("ret 16");
        }
        #[unsafe(naked)]
        unsafe extern "C" fn preserve_registers() {
            core::arch::naked_asm!("ret");
        }

        #[test]
        fn optional_fetch_supplies_the_argument_word_in_edx_to_a_preserving_callee() {
            let mut vt = [0usize; 0x14C / 4];
            vt[0x148 / 4] = preserve_four_words as *const () as usize;
            let mut object = [vt.as_ptr() as usize];
            let mut output = 0x1111_2222u32;
            let out = (&mut output as *mut u32) as u32;
            let result = unsafe {
                run(
                    fc4p_00657740 as *const (),
                    Conv::Thiscall,
                    object.as_mut_ptr().cast(),
                    &[0x8765_4321, out, 3, 4],
                    0x2222_3333,
                    0x4444_5555,
                )
            };
            assert_eq!(result, (out, 0x8765_4321));
            assert_eq!(output, out);
        }

        extern "C" fn search(first: *mut u8, byte: u32, length: u32) -> u64 {
            log(0x009F_0B40, first, &[byte, length]);
            let bytes = unsafe { core::slice::from_raw_parts(first, length as usize) };
            let found = bytes
                .iter()
                .position(|&b| b == byte as u8)
                .map_or(0, |i| first.wrapping_add(i) as u32);
            registers(found, 0xDEAD_BEEF)
        }

        #[test]
        fn line_scanner_skips_empty_lines_trims_one_carriage_return_and_keeps_unterminated_data() {
            reset();
            let _find = fake(0x009F_0B40, search as *const () as usize);
            let mut input = *b"\n\r\nabc\r\nxyz";
            let start = input.as_mut_ptr();
            let end = start.wrapping_add(input.len()) as u32;
            let mut object = [0xA5A5_A5A5u32; 6];
            object[3] = start as u32;
            object[4] = end;
            let p = object.as_mut_ptr().cast::<u8>();
            let mut first = 0x1111_1111u32;
            let mut last = 0x2222_2222u32;
            let args = [
                (&mut first as *mut u32) as u32,
                (&mut last as *mut u32) as u32,
            ];
            let one = unsafe {
                run(
                    fc4p_0067c240 as *const (),
                    Conv::Thiscall,
                    p,
                    &args,
                    0,
                    0x3333_3333,
                )
            };
            assert_eq!(
                (first, last, object[3]),
                (
                    start.wrapping_add(3) as u32,
                    start.wrapping_add(6) as u32,
                    start.wrapping_add(8) as u32
                )
            );
            assert_eq!(one, (((last & !0xFF) | 1), last));
            let two = unsafe { run(fc4p_0067c240 as *const (), Conv::Thiscall, p, &args, 0, 0) };
            assert_eq!(
                (first, last, object[3]),
                (start.wrapping_add(8) as u32, end, end)
            );
            assert_eq!(two, ((((end - 1) & !0xFF) | 1), end));
            let outputs = (first, last);
            let three = unsafe {
                run(
                    fc4p_0067c240 as *const (),
                    Conv::Thiscall,
                    p,
                    &args,
                    0x1111_2222,
                    0x3333_4444,
                )
            };
            assert_eq!(three, (end & !0xFF, 0x3333_4444));
            assert_eq!((first, last), outputs);
            assert_eq!(&object[..3], &[0xA5A5_A5A5; 3]);
            assert_eq!(object[5], 0xA5A5_A5A5);
            assert_eq!(input, *b"\n\r\nabc\r\nxyz");
            assert_eq!(
                calls(),
                [
                    Call(0x009F_0B40, start as usize, vec![10, 11]),
                    Call(0x009F_0B40, start.wrapping_add(1) as usize, vec![10, 10]),
                    Call(0x009F_0B40, start.wrapping_add(3) as usize, vec![10, 8]),
                    Call(0x009F_0B40, start.wrapping_add(8) as usize, vec![10, 3]),
                ]
            );
        }

        #[test]
        fn batch_adjustment_checks_thresholds_bounds_and_signed_counter_wrap() {
            for sample in [0u32, 1, 4, 5, 20, 21, u32::MAX >> 10] {
                for size in [0u32, 1, 2, 1023, 1024, u32::MAX] {
                    for count in [i32::MIN, -5, -4, -3, 0, 14, 15, 16, i32::MAX] {
                        let mut state = [size, count as u32, 0x5A5A_5A5A];
                        let st = state.as_mut_ptr() as u32;
                        let mut object = [0xA5A5_A5A5u32; 7];
                        object[2] = sample << 10;
                        object[5] = st;
                        let before = object;
                        let mut expected = state;
                        let mut result = (sample, 0);
                        if sample > 20 || (sample != 0 && sample < 5) {
                            expected[1] = if sample > 20 {
                                count.wrapping_add(1) as u32
                            } else {
                                count.wrapping_sub(1) as u32
                            };
                            result = (st, 0);
                            if (sample > 20 && expected[1] as i32 >= 16)
                                || (sample < 5 && expected[1] as i32 <= -4)
                            {
                                expected[1] = 0;
                                result = (st, st);
                                if sample > 20 && size > 1 {
                                    expected[0] = size >> 1;
                                    result.0 = expected[0];
                                }
                                if sample < 5 && size < 1024 {
                                    expected[0] = size << 1;
                                    result.0 = expected[0];
                                }
                            }
                        }
                        let actual = unsafe { fc4p_0068b960(object.as_mut_ptr().cast()) };
                        assert_eq!(
                            actual,
                            registers(result.0, result.1),
                            "sample={sample} size={size} count={count}"
                        );
                        assert_eq!(state, expected);
                        assert_eq!(object, before);
                    }
                }
            }
        }

        extern "C" fn allocate(size: u32) -> *mut u8 {
            log(0x0090_CF54, core::ptr::null_mut(), &[size]);
            FIXTURE.with(|s| s.borrow().pointers[0] as *mut u8)
        }
        extern "C" fn copy(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 {
            log(
                0x0077_AC20,
                first,
                &[last as u32, out as u32, unsafe { cookie.read_unaligned() }],
            );
            unsafe {
                core::ptr::copy_nonoverlapping(
                    first,
                    out,
                    (last as usize).wrapping_sub(first as usize),
                );
            }
            registers(0x1111_2222, 0x3333_4444)
        }
        extern "thiscall" fn destroy(this: *mut u8) -> u64 {
            log(0x005F_F0C0, this, &[]);
            FIXTURE.with(|s| s.borrow_mut().snapshots.push(bytes(this, 12)));
            unsafe {
                put(this, 0, 0);
                put(this, 4, 0);
                put(this, 8, 0);
            }
            registers(0x5555_6666, 0x7777_8888)
        }

        #[test]
        fn reserve_pairs_preserves_noop_registers_and_copies_before_destroying_old_storage() {
            reset();
            let _alloc = fake(0x0090_CF54, allocate as *const () as usize);
            let _copy = fake(0x0077_AC20, copy as *const () as usize);
            let _destroy = fake(0x005F_F0C0, destroy as *const () as usize);
            let mut old = [1u32, 2, 3, 4, 0xABCD, 0xEF01];
            let mut new = [0xA5A5_A5A5u32; 10];
            let a = old.as_mut_ptr().cast::<u8>();
            let b = new.as_mut_ptr().cast::<u8>();
            FIXTURE.with(|s| {
                s.borrow_mut().pointers[0] = b as usize;
                s.borrow_mut().snapshots.clear();
            });
            let mut vector = [
                a as u32,
                a.wrapping_add(16) as u32,
                a.wrapping_add(24) as u32,
                0xA5A5_A5A5,
            ];
            let p = vector.as_mut_ptr().cast::<u8>();
            let before = vector;
            for request in [0, 2, 3] {
                let result = unsafe {
                    run(
                        fc4p_0067cb20 as *const (),
                        Conv::Thiscall,
                        p,
                        &[request],
                        0xFFFF_FFFF,
                        0x1234_5678,
                    )
                };
                assert_eq!(result, (3, 0x1234_5678));
                assert_eq!(vector, before);
                assert!(calls().is_empty());
            }
            let result = unsafe { run(fc4p_0067cb20 as *const (), Conv::Thiscall, p, &[5], 0, 0) };
            assert_eq!(
                result,
                (b.wrapping_add(40) as u32, b.wrapping_add(16) as u32)
            );
            assert_eq!(
                vector,
                [
                    b as u32,
                    b.wrapping_add(16) as u32,
                    b.wrapping_add(40) as u32,
                    0xA5A5_A5A5
                ]
            );
            assert_eq!(
                new,
                [
                    1,
                    2,
                    3,
                    4,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5
                ]
            );
            assert_eq!(
                calls(),
                [
                    Call(0x0090_CF54, 0, vec![40]),
                    Call(
                        0x0077_AC20,
                        a as usize,
                        vec![a.wrapping_add(16) as u32, b as u32, 5]
                    ),
                    Call(0x005F_F0C0, p as usize, vec![])
                ]
            );
            FIXTURE.with(|s| {
                assert_eq!(
                    s.borrow().snapshots,
                    [before[..3]
                        .iter()
                        .flat_map(|w| w.to_le_bytes())
                        .collect::<Vec<_>>()]
                )
            });
            vector[..3].fill(0);
            let result = unsafe { run(fc4p_0067cb20 as *const (), Conv::Thiscall, p, &[2], 0, 0) };
            assert_eq!(result, (b.wrapping_add(16) as u32, b as u32));
            assert_eq!(calls(), [Call(0x0090_CF54, 0, vec![16])]);
            vector[..3].fill(0);
            let result = unsafe {
                run(
                    fc4p_0067cb20 as *const (),
                    Conv::Thiscall,
                    p,
                    &[0x2000_0000],
                    0,
                    0,
                )
            };
            assert_eq!(result, (b as u32, b as u32));
            assert_eq!(calls(), [Call(0x0090_CF54, 0, vec![0])]);
        }

        extern "C" fn backward(first: *mut u8, last: *mut u8, out: *mut u8, cookie: *mut u32) -> u64 {
            log(0x0066_B340, first, &[last as u32, out as u32]);
            unsafe {
                core::ptr::copy(
                    first,
                    out.wrapping_sub((last as usize).wrapping_sub(first as usize)),
                    (last as usize).wrapping_sub(first as usize),
                );
                cookie.write_unaligned(0);
            }
            registers(0x1234_5678, 0x8765_4321)
        }
        extern "thiscall" fn grow(
            this: *mut u8,
            at: *mut u8,
            value: *const u32,
            cookie: *mut u32,
            count: u32,
            mode: u32,
        ) -> u64 {
            log(
                0x007C_61A0,
                this,
                &[
                    at as u32,
                    unsafe { value.read_unaligned() },
                    unsafe { value.add(1).read_unaligned() },
                    unsafe { cookie.read_unaligned() },
                    count,
                    mode,
                ],
            );
            FIXTURE.with(|s| {
                let p = s.borrow().pointers[0] as u32;
                unsafe {
                    put(this, 0, p);
                    put(this, 4, p.wrapping_add(24));
                    put(this, 8, p.wrapping_add(40));
                }
            });
            registers(0xABCD_EF01, 0x7654_3210)
        }

        #[test]
        fn pair_insertion_handles_append_middle_aliasing_and_growth() {
            reset();
            let _back = fake(0x0066_B340, backward as *const () as usize);
            let _grow = fake(0x007C_61A0, grow as *const () as usize);
            for index in [0usize, 1, 2] {
                let mut data = [
                    1u32,
                    2,
                    3,
                    4,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                    0xA5A5_A5A5,
                ];
                let base = data.as_mut_ptr().cast::<u8>();
                let mut vector = [
                    base as u32,
                    base.wrapping_add(16) as u32,
                    base.wrapping_add(32) as u32,
                ];
                let p = vector.as_mut_ptr().cast::<u8>();
                let at = base.wrapping_add(index * 8);
                let source = base.wrapping_add(8);
                let result = unsafe {
                    run(
                        fc4p_00665db0 as *const (),
                        Conv::Thiscall,
                        p,
                        &[at as u32, source as u32],
                        0,
                        0x1111_2222,
                    )
                };
                let mut expected = vec![1u32, 2, 3, 4];
                expected.splice(index * 2..index * 2, [3, 4]);
                expected.extend([0xA5A5_A5A5; 2]);
                assert_eq!(data.as_slice(), expected);
                assert_eq!(
                    vector,
                    [
                        base as u32,
                        base.wrapping_add(24) as u32,
                        base.wrapping_add(32) as u32
                    ]
                );
                assert_eq!(
                    result,
                    (at as u32, if index == 2 { 3 } else { 0x8765_4321 })
                );
                assert_eq!(
                    calls(),
                    if index == 2 {
                        vec![]
                    } else {
                        vec![Call(
                            0x0066_B340,
                            at as usize,
                            vec![base.wrapping_add(8) as u32, base.wrapping_add(16) as u32],
                        )]
                    }
                );
            }
            let mut old = [1u32, 2, 3, 4];
            let mut new = [0u32; 10];
            let a = old.as_mut_ptr().cast::<u8>();
            let b = new.as_mut_ptr().cast::<u8>();
            FIXTURE.with(|s| s.borrow_mut().pointers[0] = b as usize);
            let mut vector = [
                a as u32,
                a.wrapping_add(16) as u32,
                a.wrapping_add(16) as u32,
            ];
            let p = vector.as_mut_ptr().cast::<u8>();
            let result = unsafe {
                run(
                    fc4p_00665db0 as *const (),
                    Conv::Thiscall,
                    p,
                    &[a.wrapping_add(8) as u32, a as u32],
                    0,
                    0,
                )
            };
            assert_eq!(result, (b.wrapping_add(8) as u32, 0x7654_3210));
            assert_eq!(
                calls(),
                [Call(
                    0x007C_61A0,
                    p as usize,
                    vec![a.wrapping_add(8) as u32, 1, 2, a as u32, 1, 0]
                )]
            );
            vector.copy_from_slice(&[0, 0, 8]);
            let result = unsafe {
                run(
                    fc4p_00665db0 as *const (),
                    Conv::Thiscall,
                    p,
                    &[0, 0],
                    0,
                    0x1234_5678,
                )
            };
            assert_eq!(result, (0, 0x1234_5678));
            assert_eq!(vector, [0, 8, 8]);
            assert!(calls().is_empty());
        }

        extern "thiscall" fn clear(this: *mut u8) -> u64 {
            log(0x005F_F160, this, &[]);
            registers(0, 0x9999_9999)
        }
        extern "thiscall" fn clone_vector(this: *mut u8, source: *mut u8) -> *mut u8 {
            log(0x005F_EA70, source, &[]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                for i in 0..3 {
                    unsafe {
                        put(this, i * 4, s.words[i]);
                    }
                }
            });
            this
        }
        extern "thiscall" fn finish(this: *mut u8) -> u64 {
            log(0x34, this, &[]);
            registers(0x1122_3344, 0x5566_7788)
        }

        #[test]
        fn parser_compaction_swaps_all_three_words_and_releases_the_old_vector() {
            reset();
            let _clear = fake(0x005F_F160, clear as *const () as usize);
            let _clone = fake(0x005F_EA70, clone_vector as *const () as usize);
            let _destroy = fake(0x005F_F0C0, destroy as *const () as usize);
            let vt = [
                0usize,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                0,
                finish as *const () as usize,
            ];
            let mut service = [vt.as_ptr() as usize];
            let sp = service.as_mut_ptr().cast::<u8>();
            FIXTURE.with(|s| {
                s.borrow_mut().words[..3].copy_from_slice(&[0x1000, 0x1010, 0x1010]);
                s.borrow_mut().snapshots.clear();
            });
            for (size, capacity, attached) in [
                (16u32, 16, false),
                (16, 23, false),
                (16, 24, false),
                (0, 8, true),
            ] {
                let mut object = [0xA5A5_A5A5u32; 0x48 / 4];
                object[0x30 / 4] = if attached { sp as u32 } else { 0 };
                object[0x34 / 4] = 0x2000;
                object[0x38 / 4] = 0x2000 + size;
                object[0x3C / 4] = 0x2000 + capacity;
                let before = object;
                let p = object.as_mut_ptr().cast::<u8>();
                let result = unsafe { run(fc4p_0067cd50 as *const (), Conv::Thiscall, p, &[], 0, 0) };
                let compacted = (size ^ capacity) & !7 != 0;
                let mut expected = before;
                let mut expected_calls = vec![Call(0x005F_F160, p.wrapping_add(0x1C) as usize, vec![])];
                if compacted {
                    expected[0x34 / 4..0x40 / 4].copy_from_slice(&[0x1000, 0x1010, 0x1010]);
                    expected_calls.push(Call(0x005F_EA70, p.wrapping_add(0x34) as usize, vec![]));
                }
                let actual_calls = calls();
                assert_eq!(&actual_calls[..expected_calls.len()], expected_calls);
                let mut suffix = &actual_calls[expected_calls.len()..];
                if compacted {
                    assert_eq!(suffix[0].0, 0x005F_F0C0);
                    suffix = &suffix[1..];
                    FIXTURE.with(|s| {
                        assert_eq!(
                            s.borrow().snapshots.last().unwrap(),
                            &before[0x34 / 4..0x40 / 4]
                                .iter()
                                .flat_map(|v| v.to_le_bytes())
                                .collect::<Vec<_>>()
                        )
                    });
                }
                assert_eq!(
                    suffix,
                    if attached {
                        vec![Call(0x34, sp as usize, vec![])]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(object, expected);
                assert_eq!(
                    result,
                    if attached {
                        (0x1122_3344, 0x5566_7788)
                    } else {
                        (0, if compacted { 0x7777_8888 } else { 0x2000 })
                    }
                );
            }
        }

        #[test]
        fn parser_compaction_supplies_the_old_limit_to_a_preserving_destructor() {
            reset();
            let _clear = fake(0x005F_F160, clear as *const () as usize);
            let _clone = fake(0x005F_EA70, clone_vector as *const () as usize);
            let _destroy = fake(0x005F_F0C0, preserve_registers as *const () as usize);
            let mut object = [0u32; 0x40 / 4];
            object[0x3C / 4] = 8;
            let result = unsafe {
                run(
                    fc4p_0067cd50 as *const (),
                    Conv::Thiscall,
                    object.as_mut_ptr().cast(),
                    &[],
                    0,
                    0xABCD_EF01,
                )
            };
            assert_eq!(result, (0, 8));
            assert_eq!(&object[0x34 / 4..], &[0, 0, 0]);
        }

        extern "thiscall" fn read_line(this: *mut u8, first: *mut *mut u8, last: *mut *mut u8) -> u64 {
            log(0x0067_C240, this, &[]);
            FIXTURE.with(|s| {
                let s = s.borrow();
                unsafe {
                    first.write_unaligned(s.pointers[0] as *mut u8);
                    last.write_unaligned(s.pointers[1] as *mut u8);
                }
                registers(s.words[0], 0x1234_5678)
            })
        }
        extern "thiscall" fn assign(this: *mut u8, first: *mut u8, last: *mut u8) -> u64 {
            log(0x0044_D3A0, this, &[first as u32, last as u32]);
            registers(0x5060_7080, 0x1020_3040)
        }
        extern "thiscall" fn trim(this: *mut u8) -> u64 {
            log(0x0090_E8A6, this, &[]);
            registers(0xAABB_CC00, 0xDEAD_BEEF)
        }

        #[test]
        fn key_value_reader_keeps_failures_and_trims_each_side_of_the_first_equals() {
            reset();
            let _line = fake(0x0067_C240, read_line as *const () as usize);
            let _find = fake(0x009F_0B40, search as *const () as usize);
            let _assign = fake(0x0044_D3A0, assign as *const () as usize);
            let _trim = fake(0x0090_E8A6, trim as *const () as usize);
            let this = 0x1000 as *mut u8;
            let key = 0x2000 as *mut u8;
            let value = 0x3000 as *mut u8;
            for (text, available, expected) in [
                (
                    " a = b=c ",
                    0x1122_3300,
                    registers(0x1122_3300, 0x1234_5678),
                ),
                ("missing", 1, registers(0, 0xDEAD_BEEF)),
                (" a = b=c ", 1, registers(0xAABB_CC01, 0xDEAD_BEEF)),
                ("=", 1, registers(0xAABB_CC01, 0xDEAD_BEEF)),
            ] {
                let mut input = text.as_bytes().to_vec();
                let first = input.as_mut_ptr();
                let last = first.wrapping_add(input.len());
                FIXTURE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.words[0] = available;
                    s.pointers[0] = first as usize;
                    s.pointers[1] = last as usize;
                });
                let result = unsafe {
                    run(
                        fc4p_0067e0b0 as *const (),
                        Conv::Thiscall,
                        this,
                        &[key as u32, value as u32],
                        0,
                        0,
                    )
                };
                assert_eq!(result, (expected as u32, (expected >> 32) as u32));
                let mut trace = vec![Call(0x0067_C240, this as usize, vec![])];
                if available as u8 != 0 {
                    trace.push(Call(
                        0x009F_0B40,
                        first as usize,
                        vec![61, input.len() as u32],
                    ));
                    if let Some(i) = input.iter().position(|&v| v == b'=') {
                        let equals = first.wrapping_add(i);
                        trace.extend([
                            Call(
                                0x0044_D3A0,
                                key.wrapping_add(4) as usize,
                                vec![first as u32, equals as u32],
                            ),
                            Call(0x0090_E8A6, key as usize, vec![]),
                            Call(
                                0x0044_D3A0,
                                value.wrapping_add(4) as usize,
                                vec![equals.wrapping_add(1) as u32, last as u32],
                            ),
                            Call(0x0090_E8A6, value as usize, vec![]),
                        ]);
                    }
                }
                assert_eq!(calls(), trace);
                assert_eq!(input, text.as_bytes());
            }
        }

        #[unsafe(naked)]
        unsafe extern "C" fn record_line_registers() {
            core::arch::naked_asm!(
                "mov [ecx + 4], eax",
                "mov [ecx + 8], edx",
                "mov eax, [esp + 4]",
                "mov [ecx + 12], eax",
                "mov eax, [esp + 8]",
                "mov [ecx + 16], eax",
                "mov eax, 0x11223300",
                "ret 8",
            );
        }

        #[test]
        fn key_value_reader_retains_the_original_local_addresses_and_preserved_trim_result() {
            reset();
            let mut object = [0u32; 5];
            let _line = fake(0x0067_C240, record_line_registers as *const () as usize);
            let result = unsafe {
                run(
                    fc4p_0067e0b0 as *const (),
                    Conv::Thiscall,
                    object.as_mut_ptr().cast(),
                    &[0, 0],
                    0,
                    0,
                )
            };
            assert_eq!(result, (0x1122_3300, object[2]));
            assert_eq!(object[1], object[4]);
            assert_eq!(object[2], object[3]);
            assert_eq!(object[2], object[1] + 4);
            drop(_line);
            let _line = fake(0x0067_C240, read_line as *const () as usize);
            let _find = fake(0x009F_0B40, search as *const () as usize);
            let _assign = fake(0x0044_D3A0, assign as *const () as usize);
            let _trim = fake(0x0090_E8A6, preserve_registers as *const () as usize);
            let mut input = *b"=";
            FIXTURE.with(|s| {
                let mut s = s.borrow_mut();
                s.words[0] = 1;
                s.pointers[0] = input.as_mut_ptr() as usize;
                s.pointers[1] = input.as_mut_ptr().wrapping_add(1) as usize;
            });
            let result = unsafe {
                run(
                    fc4p_0067e0b0 as *const (),
                    Conv::Thiscall,
                    core::ptr::null_mut(),
                    &[0x2000, 0x3000],
                    0,
                    0,
                )
            };
            assert_eq!(result, (0x5060_7001, 0x1020_3040));
        }

        extern "thiscall" fn class(this: *mut u8) -> u64 {
            log(0x1C, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().words[0], 0x1234_5678))
        }
        extern "thiscall" fn release_object(this: *mut u8) -> u64 {
            log(8, this, &[]);
            registers(0xAABB_CCDD, 0xEEFF_0011)
        }
        extern "thiscall" fn erase(this: *mut u8, output: *mut u32, node: *mut u8) {
            log(0x0041_3F70, this, &[node as u32]);
            unsafe {
                output.write_unaligned(0);
            }
        }

        #[test]
        fn retained_object_removal_distinguishes_wrong_class_missing_and_matching_members() {
            reset();
            let _erase = fake(0x0041_3F70, erase as *const () as usize);
            let mut vt = [0usize; 8];
            vt[2] = release_object as *const () as usize;
            vt[7] = class as *const () as usize;
            let mut object = [vt.as_ptr() as usize];
            let o = object.as_mut_ptr().cast::<u8>();
            let mut header = [0u32; 3];
            let h = header.as_mut_ptr().cast::<u8>();
            let mut first = [h as u32, h as u32, o as u32];
            let n = first.as_mut_ptr().cast::<u8>();
            let mut owner = [0xA5A5_A5A5u32; 10];
            owner[7] = h as u32;
            let p = owner.as_mut_ptr().cast::<u8>();
            for (kind, member) in [(0u32, true), (0x2781_28A0, false), (0x2781_28A0, true)] {
                header[0] = if member { n as u32 } else { h as u32 };
                FIXTURE.with(|s| s.borrow_mut().words[0] = kind);
                let before = owner;
                let result = unsafe {
                    run(
                        fc4p_006acb60 as *const (),
                        Conv::Thiscall,
                        p,
                        &[o as u32],
                        0,
                        0,
                    )
                };
                if kind != 0x2781_28A0 {
                    assert_eq!(result, (kind, 0x1234_5678));
                    assert_eq!(calls(), [Call(0x1C, o as usize, vec![])]);
                } else if !member {
                    assert_eq!(result, (h as u32, h as u32));
                    assert_eq!(calls(), [Call(0x1C, o as usize, vec![])]);
                } else {
                    assert_eq!(result, (0xAABB_CCDD, 0xEEFF_0011));
                    assert_eq!(
                        calls(),
                        [
                            Call(0x1C, o as usize, vec![]),
                            Call(0x0041_3F70, p.wrapping_add(0x1C) as usize, vec![n as u32]),
                            Call(8, o as usize, vec![])
                        ]
                    );
                }
                assert_eq!(owner, before);
            }
        }

        #[test]
        fn retained_object_class_call_preserves_the_incoming_edx_word() {
            let mut vt = [0usize; 8];
            vt[7] = preserve_registers as *const () as usize;
            let mut object = [vt.as_ptr() as usize];
            let result = unsafe {
                run(
                    fc4p_006acb60 as *const (),
                    Conv::Thiscall,
                    core::ptr::null_mut(),
                    &[object.as_mut_ptr() as u32],
                    0,
                    0x1234_5678,
                )
            };
            assert_eq!(result, (vt.as_ptr() as u32, 0x1234_5678));
        }

        extern "thiscall" fn insert(
            this: *mut u8,
            output: *mut *mut u8,
            hint: *mut u8,
            pair: *const u32,
        ) -> u64 {
            log(
                0x0069_7530,
                this,
                &[hint as u32, unsafe { pair.read_unaligned() }, unsafe {
                    pair.add(1).read_unaligned()
                }],
            );
            FIXTURE.with(|s| unsafe {
                output.write_unaligned(s.borrow().pointers[0] as *mut u8);
            });
            registers(0x1122_3344, 0x5566_7788)
        }

        #[test]
        fn integer_map_access_uses_signed_lower_bound_and_supplies_zero_for_new_values() {
            reset();
            let _insert = fake(0x0069_7530, insert as *const () as usize);
            let mut header = [0u32; 6];
            let h = header.as_mut_ptr().cast::<u8>();
            let mut nodes = [[0u32; 6]; 3];
            let pointers: Vec<_> = nodes.iter_mut().map(|n| n.as_mut_ptr() as u32).collect();
            let keys = [i32::MIN, -1, i32::MAX];
            unsafe {
                put(h, 4, pointers[1]);
            }
            nodes[1][2] = pointers[0];
            nodes[1][3] = pointers[2];
            for (node, key) in nodes.iter_mut().zip(keys) {
                node[4] = key as u32;
                node[5] = 0xA5A5_A5A5;
            }
            let mut tree = [h as u32];
            let p = tree.as_mut_ptr().cast::<u8>();
            FIXTURE.with(|s| s.borrow_mut().pointers[0] = 0x1234_5600);
            let before = nodes;
            for key in [i32::MIN, i32::MIN + 1, -2, -1, 0, i32::MAX] {
                let result = unsafe {
                    run(
                        fc4p_006b5840 as *const (),
                        Conv::Thiscall,
                        p,
                        &[(&key as *const i32) as u32],
                        0,
                        0,
                    )
                };
                if let Some(i) = keys.iter().position(|&k| k == key) {
                    assert_eq!(result, (pointers[i] + 0x14, pointers[i]));
                    assert!(calls().is_empty());
                } else {
                    let hint = keys
                        .iter()
                        .position(|&k| k >= key)
                        .map_or(h as u32, |i| pointers[i]);
                    assert_eq!(result, (0x1234_5614, 0x5566_7788));
                    assert_eq!(
                        calls(),
                        [Call(0x0069_7530, p as usize, vec![hint, key as u32, 0])]
                    );
                }
                assert_eq!(nodes, before);
            }
            unsafe {
                put(h, 4, 0);
            }
            let key = 0i32;
            unsafe {
                assert_eq!(
                    run(
                        fc4p_006b5840 as *const (),
                        Conv::Thiscall,
                        p,
                        &[(&key as *const i32) as u32],
                        0,
                        0
                    ),
                    (0x1234_5614, 0x5566_7788)
                );
            }
            assert_eq!(
                calls(),
                [Call(0x0069_7530, p as usize, vec![h as u32, 0, 0])]
            );
        }
        extern "thiscall" fn notify_manager(this: *mut u8, object: *mut u8) -> u64 {
            log(0x80, this, &[object as u32]);
            registers(0x1111_2222, 0x3333_4444)
        }

        struct Lists {
            tool: Vec<u32>,
            headers: Vec<[u32; 3]>,
            nodes: Vec<[u32; 3]>,
            objects: Vec<[u32; 3]>,
            split: usize,
        }
        impl Lists {
            fn new(primary: usize, secondary: usize, object_vt: usize, interface_vt: usize) -> Self {
                let mut tool = vec![0xA5A5_A5A5; 0x390 / 4];
                let mut headers = vec![[0u32, 0, 0xABCD_EF01]; 2];
                let objects =
                    vec![[object_vt as u32, interface_vt as u32, 0xCAFE_BABE]; primary + secondary];
                let mut nodes = vec![[0u32; 3]; objects.len()];
                for (i, (begin, count)) in [(0, primary), (primary, secondary)].into_iter().enumerate()
                {
                    let header = headers[i].as_mut_ptr() as u32;
                    let (first, last) = if count == 0 {
                        (header, header)
                    } else {
                        (
                            nodes[begin].as_ptr() as u32,
                            nodes[begin + count - 1].as_ptr() as u32,
                        )
                    };
                    headers[i][0] = first;
                    headers[i][1] = last;
                    for n in begin..begin + count {
                        nodes[n] = [
                            if n + 1 == begin + count {
                                header
                            } else {
                                nodes[n + 1].as_ptr() as u32
                            },
                            0x1122_3344,
                            objects[n].as_ptr() as u32,
                        ];
                    }
                }
                tool[0x48 / 4] = 0;
                tool[0x208 / 4] = 0x9876_5432;
                tool[0x368 / 4] = 0xA5A5_A580;
                tool[0x384 / 4] = headers[0].as_ptr() as u32;
                tool[0x388 / 4] = headers[1].as_ptr() as u32;
                Self {
                    tool,
                    headers,
                    nodes,
                    objects,
                    split: primary,
                }
            }
            fn ptr(&mut self) -> *mut u8 {
                self.tool.as_mut_ptr().cast()
            }
            fn assert_empty(&self) {
                for header in &self.headers {
                    assert_eq!(
                        *header,
                        [header.as_ptr() as u32, header.as_ptr() as u32, 0xABCD_EF01]
                    );
                }
            }
            fn releases(&self, begin: usize, end: usize) -> Vec<Call> {
                let mut expected = self.objects[begin..end]
                    .iter()
                    .map(|o| Call(8, o.as_ptr() as usize, vec![]))
                    .collect::<Vec<_>>();
                expected.extend(
                    self.nodes[begin..end]
                        .iter()
                        .map(|n| Call(0x0090_CF63, n.as_ptr() as usize, vec![])),
                );
                expected
            }
        }
        extern "C" fn free_link(node: *mut u8) -> u64 {
            log(0x0090_CF63, node, &[]);
            registers(0xF0F0_ABCD, 0xDADA_CCCC)
        }
        extern "thiscall" fn stop_base(this: *mut u8) -> u64 {
            log(0x0062_6D30, this, &[]);
            registers(0xFACE_B080, 0xDEAD_BEEF)
        }
        extern "thiscall" fn reset_base(this: *mut u8) -> u64 {
            log(0x0062_5580, this, &[]);
            registers(0xFACE_B080, 0xDEAD_BEEF)
        }
        extern "thiscall" fn detach(this: *mut u8, a: u32, b: u32) -> u64 {
            log(0x44, this, &[a, b]);
            registers(0xABCD_0044, 0xDCBA_0044)
        }
        extern "thiscall" fn stop_object(this: *mut u8) -> u64 {
            log(0x10, this, &[]);
            registers(0xABCD_0010, 0xDCBA_0010)
        }
        extern "thiscall" fn object_model(this: *mut u8) -> u64 {
            log(0xC0, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().pointers[0] as u32, 0xDCBA_00C0))
        }
        extern "thiscall" fn model_flags(this: *mut u8) -> u64 {
            log(0x84, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().words[0], 0xDCBA_0084))
        }
        extern "thiscall" fn remove_occupant(
            this: *mut u8,
            interface: *mut u8,
            zero: u32,
            context: u32,
        ) -> u64 {
            log(0x54, this, &[interface as u32, zero, context]);
            registers(0xABCD_0054, 0xDCBA_0054)
        }

        #[test]
        fn connection_shutdown_releases_each_list_before_freeing_its_links_and_calls_the_base_last() {
            reset();
            let _free = fake(0x0090_CF63, free_link as *const () as usize);
            let _base = fake(0x0062_6D30, stop_base as *const () as usize);
            let vt = [0usize, 0, release_object as *const () as usize];
            let mut inactive = [0xA5A5_A5A5u32; 0x390 / 4];
            inactive[0x368 / 4] &= !0xFF;
            let before = inactive;
            let result = unsafe {
                run(
                    fc4p_00657af0 as *const (),
                    Conv::Thiscall,
                    inactive.as_mut_ptr().cast(),
                    &[],
                    0x1234_56FF,
                    0x8765_4321,
                )
            };
            assert_eq!(result, (0x1234_5601, 0x8765_4321));
            assert_eq!(inactive, before);
            assert!(calls().is_empty());
            for counts in [(0, 0), (2, 1), (0, 2), (1, 0)] {
                let mut lists = Lists::new(counts.0, counts.1, vt.as_ptr() as usize, 0);
                let p = lists.ptr();
                let mut before = lists.tool.clone();
                before[0x368 / 4] &= !0xFF;
                let result = unsafe { run(fc4p_00657af0 as *const (), Conv::Thiscall, p, &[], 0, 0) };
                assert_eq!(result, (0xFACE_B080, 0xDEAD_BEEF));
                let mut expected = lists.releases(0, lists.split);
                expected.extend(lists.releases(lists.split, lists.objects.len()));
                expected.push(Call(0x0062_6D30, p as usize, vec![]));
                assert_eq!(calls(), expected);
                assert_eq!(lists.tool, before);
                lists.assert_empty();
            }
            drop(_base);
            let _base = fake(0x0062_6D30, preserve_registers as *const () as usize);
            let mut lists = Lists::new(0, 0, 0, 0);
            let result = unsafe {
                run(
                    fc4p_00657af0 as *const (),
                    Conv::Thiscall,
                    lists.ptr(),
                    &[],
                    0,
                    0x1234_5678,
                )
            };
            assert_eq!(result, (lists.headers[1].as_ptr() as u32, 0x1234_5678));
        }

        #[test]
        fn connection_reset_detaches_both_lists_and_removes_only_primary_models_with_bit_zero_set() {
            reset();
            let _free = fake(0x0090_CF63, free_link as *const () as usize);
            let _base = fake(0x0062_5580, reset_base as *const () as usize);
            let mut primary_vt = [0usize; 0xC4 / 4];
            primary_vt[2] = release_object as *const () as usize;
            primary_vt[4] = stop_object as *const () as usize;
            primary_vt[0xC0 / 4] = object_model as *const () as usize;
            let mut draw_vt = [0usize; 0x48 / 4];
            draw_vt[0x44 / 4] = detach as *const () as usize;
            let mut model_vt = [0usize; 0x88 / 4];
            model_vt[0x84 / 4] = model_flags as *const () as usize;
            let mut model = [model_vt.as_ptr() as usize];
            let model_ptr = model.as_mut_ptr().cast::<u8>();
            let mut service_vt = [0usize; 0x58 / 4];
            service_vt[0x54 / 4] = remove_occupant as *const () as usize;
            let mut service = [service_vt.as_ptr() as usize];
            let sp = service.as_mut_ptr().cast::<u8>();
            for (attached, flags) in [(false, 1u32), (true, 0x80), (true, 0), (true, 1)] {
                for counts in [(0, 0), (2, 1)] {
                    let mut lists = Lists::new(
                        counts.0,
                        counts.1,
                        primary_vt.as_ptr() as usize,
                        draw_vt.as_ptr() as usize,
                    );
                    lists.tool[0x48 / 4] = if attached { sp as u32 } else { 0 };
                    FIXTURE.with(|s| {
                        s.borrow_mut().pointers[0] = model_ptr as usize;
                        s.borrow_mut().words[0] = flags;
                    });
                    let p = lists.ptr();
                    let before = lists.tool.clone();
                    let result =
                        unsafe { run(fc4p_00657be0 as *const (), Conv::Thiscall, p, &[], 0, 0) };
                    assert_eq!(result, (0xFACE_B080, 0xDEAD_BEEF));
                    let mut expected = vec![];
                    for object in &lists.objects[..lists.split] {
                        let op = object.as_ptr() as usize;
                        expected.push(Call(0x44, op + 4, vec![0, 0]));
                        if attached {
                            expected.extend([
                                Call(0xC0, op, vec![]),
                                Call(0x84, model_ptr as usize, vec![]),
                            ]);
                            if flags & 1 != 0 {
                                expected.push(Call(
                                    0x54,
                                    sp as usize,
                                    vec![(op + 4) as u32, 0, 0x9876_5432],
                                ));
                            }
                        }
                        expected.extend([Call(0x10, op, vec![]), Call(8, op, vec![])]);
                    }
                    expected.extend(
                        lists.nodes[..lists.split]
                            .iter()
                            .map(|n| Call(0x0090_CF63, n.as_ptr() as usize, vec![])),
                    );
                    for object in &lists.objects[lists.split..] {
                        let op = object.as_ptr() as usize;
                        expected.extend([Call(0x44, op + 4, vec![0, 0]), Call(8, op, vec![])]);
                    }
                    expected.extend(
                        lists.nodes[lists.split..]
                            .iter()
                            .map(|n| Call(0x0090_CF63, n.as_ptr() as usize, vec![])),
                    );
                    expected.push(Call(0x0062_5580, p as usize, vec![]));
                    assert_eq!(calls(), expected);
                    assert_eq!(lists.tool, before);
                    lists.assert_empty();
                }
            }
        }
        extern "C" fn message_allocate(size: u32) -> u64 {
            log(0x0091_33DA, core::ptr::null_mut(), &[size]);
            FIXTURE.with(|s| registers(s.borrow().pointers[0] as u32, 0x5555_6666))
        }
        extern "thiscall" fn message_construct(this: *mut u8) -> u64 {
            log(0x0091_34D6, this, &[]);
            FIXTURE.with(|s| registers(s.borrow().pointers[1] as u32, 0x7777_8888))
        }
        macro_rules! message_slot {
            ($name:ident, $slot:literal) => {
                extern "thiscall" fn $name(this: *mut u8, value: u32) -> u64 {
                    log($slot, this, &[value]);
                    registers(0xAABB_0000 | $slot, 0xCCDD_0000 | $slot)
                }
            };
        }
        message_slot!(message_id, 0x14);
        message_slot!(message_association, 0x84);
        message_slot!(message_payload, 0x40);
        message_slot!(message_zero, 0x54);
        extern "thiscall" fn message_attach(this: *mut u8, object: *mut u8) -> u64 {
            log(0x30, this, &[object as u32]);
            unsafe {
                put(object, 0x10, 0xFEDC_BA98);
            }
            registers(0xAABB_0030, 0xCCDD_0030)
        }
        extern "thiscall" fn message_add_ref(this: *mut u8) -> u64 {
            log(4, this, &[]);
            registers(0xAABB_0004, 0xCCDD_0004)
        }
        extern "thiscall" fn post_message(this: *mut u8, message: *mut u8, zero: u32) -> u64 {
            log(0x10, this, &[message as u32, zero]);
            registers(0xAABB_0010, 0xCCDD_0010)
        }

        #[test]
        fn object_event_configures_the_constructed_message_and_posts_before_releasing() {
            reset();
            let _alloc = fake(0x0091_33DA, message_allocate as *const () as usize);
            let _ctor = fake(0x0091_34D6, message_construct as *const () as usize);
            let mut manager_vt = [0usize; 0x84 / 4];
            manager_vt[0x80 / 4] = notify_manager as *const () as usize;
            let mut manager = [manager_vt.as_ptr() as usize];
            let mut manager_pointer = manager.as_mut_ptr().cast::<u8>();
            let _manager =
                crate::testing::fake_data(0x00B4_3D64, (&mut manager_pointer as *mut *mut u8).cast());
            let mut server_vt = [0usize; 0x14 / 4];
            server_vt[0x10 / 4] = post_message as *const () as usize;
            let mut server = [server_vt.as_ptr() as usize];
            let mut server_pointer = server.as_mut_ptr().cast::<u8>();
            let _server =
                crate::testing::fake_data(0x00B4_3CCC, (&mut server_pointer as *mut *mut u8).cast());
            let mut message_vt = [0usize; 0x88 / 4];
            message_vt[1] = message_add_ref as *const () as usize;
            message_vt[2] = release_object as *const () as usize;
            message_vt[0x14 / 4] = message_id as *const () as usize;
            message_vt[0x30 / 4] = message_attach as *const () as usize;
            message_vt[0x84 / 4] = message_association as *const () as usize;
            message_vt[0x40 / 4] = message_payload as *const () as usize;
            message_vt[0x54 / 4] = message_zero as *const () as usize;
            let mut message = [0xA5A5_A5A5u32; 0x2C / 4];
            message[0] = message_vt.as_ptr() as u32;
            let msg = message.as_mut_ptr().cast::<u8>();
            let mut allocation = [0x5A5A_5A5Au32; 0x2C / 4];
            let alloc = allocation.as_mut_ptr().cast::<u8>();
            let mut object = [0x1122_3344u32; 8];
            let p = object.as_mut_ptr().cast::<u8>();
            FIXTURE.with(|s| {
                s.borrow_mut().pointers[0] = alloc as usize;
                s.borrow_mut().pointers[1] = msg as usize;
            });
            for preserve_release in [false, true] {
                message_vt[2] = if preserve_release {
                    preserve_registers as *const () as usize
                } else {
                    release_object as *const () as usize
                };
                let result = unsafe {
                    run(
                        fc4p_006a9980 as *const (),
                        Conv::Thiscall,
                        p,
                        &[],
                        0,
                        0x1234_5678,
                    )
                };
                let mut expected = vec![
                    Call(0x80, manager_pointer as usize, vec![p as u32]),
                    Call(0x0091_33DA, 0, vec![0x2C]),
                    Call(0x0091_34D6, alloc as usize, vec![]),
                    Call(4, msg as usize, vec![]),
                    Call(0x14, msg as usize, vec![0x08DA_C3E8]),
                    Call(0x30, msg as usize, vec![p as u32]),
                    Call(0x84, msg as usize, vec![p as u32]),
                    Call(0x40, msg as usize, vec![0xFEDC_BA98]),
                    Call(0x54, msg as usize, vec![0]),
                    Call(0x10, server_pointer as usize, vec![msg as u32, 0]),
                ];
                if preserve_release {
                    assert_eq!(result, (0xAABB_0010, message_vt.as_ptr() as u32));
                } else {
                    expected.push(Call(8, msg as usize, vec![]));
                    assert_eq!(result, (0xAABB_CCDD, 0xEEFF_0011));
                }
                assert_eq!(calls(), expected);
                assert_eq!(allocation, [0x5A5A_5A5A; 0x2C / 4]);
                assert_eq!(object[4], 0xFEDC_BA98);
            }
        }
    }

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
