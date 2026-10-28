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
                let coordinates = pointer(this, 0x60);
                let x = word(coordinates, 0);
                let z = word(coordinates, 4);
                result = vcall!(pointer(this, 0x44), 0x88 => fn(u32, u32) -> u64, x, z);
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
            let x = pointer(a, 0);
            let y = pointer(b, 0);
            let len = word(a, 4).wrapping_sub(x as u32);
            if len != word(b, 4).wrapping_sub(y as u32) { return registers(0, b as u32); }
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
                    let nx = word(node, 0x10) as i32;
                    if x < nx || (x == nx && word(node, 0x14) as i32 >= key.add(1).read_unaligned()) {
                        result = node;
                        node = pointer(node, 8);
                    } else {
                        node = pointer(node, 0x0C);
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
                    $virtual_adjust, "sub ecx, {adjust}", "jmp dword ptr [2f]", "2: .long {target}",
                    adjust = const $adjust, target = const $target,
                );
                #[cfg(test)]
                core::arch::naked_asm!(
                    $virtual_adjust, "sub ecx, {adjust}", "push eax", "push ecx", "push edx",
                    "push {target}", "call {resolve}", "add esp, 4",
                    "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
                    adjust = const $adjust, target = const $target, resolve = sym forwarding_target,
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
    game_fn! { pub fn initialize_connection(this: *mut u8) -> u32 = "thiscall" 0x0065_4140; }
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
    game_fn! { pub fn count_ports(this: *mut u8, kind: u32) -> u32 = "thiscall" 0x006E_94C0; }
    game_fn! { pub fn get_port_lots(this: *mut u8, output: *mut u8) -> u32 = "thiscall" 0x006E_A2F0; }
    game_fn! { pub fn add_ref_at_eight(this: *mut u8) -> u32 = "thiscall" 0x004F_7BA0; }
    game_fn! { pub fn port_release(this: *mut u8) -> u32 = "thiscall" 0x006E_8F90; }
    game_fn! { pub fn landfill_release(this: *mut u8) -> u32 = "thiscall" 0x006D_5140; }
}

forward_interface!(
    fc4p_006480d0,
    0x0064_80D0,
    4,
    "",
    0x0065_4120,
    "AddRefConnectionFromSecondInterface"
);
forward_interface!(
    fc4p_006480e0,
    0x0064_80E0,
    4,
    "",
    0x0065_4130,
    "ReleaseConnectionFromSecondInterface"
);
forward_interface!(
    fc4p_006481d0,
    0x0064_81D0,
    0x24C,
    "sub ecx, [ecx - 4]",
    0x0065_4120,
    "AddRefConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_006481e0,
    0x0064_81E0,
    0x24C,
    "sub ecx, [ecx - 4]",
    0x0065_4130,
    "ReleaseConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648200,
    0x0064_8200,
    0x1E4,
    "sub ecx, [ecx - 4]",
    0x0080_1860,
    "SetModelFromVirtualBase"
);
forward_interface!(
    fc4p_00648210,
    0x0064_8210,
    0x1E4,
    "sub ecx, [ecx - 4]",
    0x0080_1890,
    "GetModelStateFromVirtualBase"
);
forward_interface!(
    fc4p_00648220,
    0x0064_8220,
    0x1E4,
    "sub ecx, [ecx - 4]",
    0x0080_18A0,
    "SetModelStateFromVirtualBase"
);
forward_interface!(
    fc4p_00648240,
    0x0064_8240,
    0,
    "sub ecx, [ecx - 4]",
    0x0065_4140,
    "InitializeConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648250,
    0x0064_8250,
    0,
    "sub ecx, [ecx - 4]",
    0x0065_4500,
    "ReadConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648260,
    0x0064_8260,
    0,
    "sub ecx, [ecx - 4]",
    0x0065_4590,
    "WriteConnectionFromVirtualBase"
);
forward_interface!(
    fc4p_00648290,
    0x0064_8290,
    0x1E4,
    "sub ecx, [ecx - 4]",
    0x0080_18E0,
    "GetModelFlagsFromVirtualBase"
);
forward_interface!(
    fc4p_006482b0,
    0x0064_82B0,
    0x1E4,
    "sub ecx, [ecx - 4]",
    0x0080_1E10,
    "CopyModelDataFromVirtualBase"
);
forward_interface!(
    fc4p_006485d0,
    0x0064_85D0,
    4,
    "",
    0x0065_A860,
    "AddRefBridgeFromSecondInterface"
);
forward_interface!(
    fc4p_006485e0,
    0x0064_85E0,
    4,
    "",
    0x0065_A870,
    "ReleaseBridgeFromSecondInterface"
);
forward_interface!(
    fc4p_00648600,
    0x0064_8600,
    8,
    "",
    0x0065_A860,
    "AddRefBridgeFromThirdInterface"
);
forward_interface!(
    fc4p_00648610,
    0x0064_8610,
    8,
    "",
    0x0065_A870,
    "ReleaseBridgeFromThirdInterface"
);
forward_interface!(
    fc4p_006491d0,
    0x0064_91D0,
    -8,
    "",
    0x005E_8DF0,
    "ReleaseOccupantAtEight"
);
forward_interface!(
    fc4p_0064dd70,
    0x0064_DD70,
    4,
    "",
    0x0064_91C0,
    "AddRefPropFromSecondInterface"
);
forward_interface!(
    fc4p_0064dd80,
    0x0064_DD80,
    4,
    "",
    0x0064_91D0,
    "ReleaseConstructionFromSecondInterface"
);
forward_interface!(
    fc4p_0064dd90,
    0x0064_DD90,
    8,
    "",
    0x0064_91C0,
    "AddRefPropFromThirdInterface"
);
forward_interface!(
    fc4p_0064dda0,
    0x0064_DDA0,
    8,
    "",
    0x0064_91D0,
    "ReleaseConstructionFromThirdInterface"
);
forward_interface!(
    fc4p_0064ddb0,
    0x0064_DDB0,
    12,
    "",
    0x0064_91C0,
    "AddRefPropFromFourthInterface"
);
forward_interface!(
    fc4p_0064ddc0,
    0x0064_DDC0,
    12,
    "",
    0x0064_91D0,
    "ReleaseConstructionFromFourthInterface"
);
forward_interface!(
    fc4p_0064ddd0,
    0x0064_DDD0,
    16,
    "",
    0x0064_91C0,
    "AddRefPropFromFifthInterface"
);
forward_interface!(
    fc4p_0064dde0,
    0x0064_DDE0,
    16,
    "",
    0x0064_91D0,
    "ReleaseConstructionFromFifthInterface"
);
forward_interface!(
    fc4p_00652e50,
    0x0065_2E50,
    4,
    "",
    0x0065_2CF0,
    "ShutdownPipeFromSecondInterface"
);
forward_interface!(
    fc4p_0068aff0,
    0x0068_AFF0,
    -4,
    "",
    0x006E_94C0,
    "CountPortsFromPreviousInterface"
);
forward_interface!(
    fc4p_0068b000,
    0x0068_B000,
    -4,
    "",
    0x006E_A2F0,
    "GetPortLotsFromPreviousInterface"
);
forward_interface!(fc4p_0068b1d0, 0x0068_B1D0, -4, "", 0x004F_7BA0, "AddRefAtNextInterface");
forward_interface!(
    fc4p_0068b1e0,
    0x0068_B1E0,
    -4,
    "",
    0x006E_8F90,
    "ReleasePortAtNextInterface"
);
forward_interface!(
    fc4p_0068d230,
    0x0068_D230,
    4,
    "",
    0x004F_7BA0,
    "AddRefFromSecondInterface"
);
forward_interface!(
    fc4p_0068d250,
    0x0068_D250,
    8,
    "",
    0x004F_7BA0,
    "AddRefFromThirdInterface"
);
forward_interface!(
    fc4p_006b8ab0,
    0x006B_8AB0,
    4,
    "",
    0x006D_5140,
    "ReleaseLandfillFromSecondInterface"
);
forward_interface!(
    fc4p_006b8ac0,
    0x006B_8AC0,
    8,
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
    fc4p_0067cb10::PORT,
    fc4p_0068afd0::PORT,
    fc4p_0068aff0::PORT,
    fc4p_0068b000::PORT,
    fc4p_0068b1d0::PORT,
    fc4p_0068b1e0::PORT,
    fc4p_0068d230::PORT,
    fc4p_0068d250::PORT,
    fc4p_00695260::PORT,
    fc4p_00695330::PORT,
    fc4p_006aaeb0::PORT,
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
}
