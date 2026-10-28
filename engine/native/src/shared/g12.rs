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

//! Shared helpers, group 12: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;

port! {
    /// Stores opaque black and returns the color address.
    #[port(address = 0x008fe5ac, owner = "shared", name = "InitializeOpaqueBlack", naked)]
    pub unsafe extern "C" fn fc4p_008fe5ac() {
        core::arch::naked_asm!("mov eax, ecx", "mov dword ptr [eax], 0xff000000", "ret");
    }
}

port! {
    /// Clears three words and returns zero.
    #[port(address = 0x009086ac, owner = "shared", name = "ClearThreeWords", naked)]
    pub unsafe extern "C" fn fc4p_009086ac() {
        core::arch::naked_asm!("xor eax, eax", "mov [ecx], eax", "mov [ecx + 4], eax", "mov [ecx + 8], eax", "ret");
    }
}

port! {
    /// Returns one in AL and removes one stack word.
    #[port(address = 0x0091559a, owner = "shared", name = "ReturnTruePopOne", naked)]
    pub unsafe extern "C" fn fc4p_0091559a() {
        core::arch::naked_asm!("mov al, 1", "ret 4");
    }
}

port! {
    /// Returns one in AL and removes three stack words.
    #[port(address = 0x0092973b, owner = "shared", name = "ReturnTruePopThree", naked)]
    pub unsafe extern "C" fn fc4p_0092973b() {
        core::arch::naked_asm!("mov al, 1", "ret 12");
    }
}

port! {
    /// Returns one in AL and removes two stack words.
    #[port(address = 0x0092b33a, owner = "shared", name = "ReturnTruePopTwo", naked)]
    pub unsafe extern "C" fn fc4p_0092b33a() {
        core::arch::naked_asm!("mov al, 1", "ret 8");
    }
}

port! {
    /// Clears AL and removes three stack words.
    #[port(address = 0x009378bc, owner = "shared", name = "ReturnFalsePopThree", naked)]
    pub unsafe extern "C" fn fc4p_009378bc() {
        core::arch::naked_asm!("xor al, al", "ret 12");
    }
}

port! {
    /// Clears AL and removes two stack words.
    #[port(address = 0x0093877e, owner = "shared", name = "ReturnFalsePopTwo", naked)]
    pub unsafe extern "C" fn fc4p_0093877e() {
        core::arch::naked_asm!("xor al, al", "ret 8");
    }
}

port! {
    /// Returns one in AL, retaining all other register bits.
    #[port(address = 0x00949ade, owner = "shared", name = "ReturnTrue", naked)]
    pub unsafe extern "C" fn fc4p_00949ade() {
        core::arch::naked_asm!("mov al, 1", "ret");
    }
}

port! {
    /// Removes one stack word and retains all registers.
    #[port(address = 0x00949b73, owner = "shared", name = "IgnoreOneArgument", naked)]
    pub unsafe extern "C" fn fc4p_00949b73() {
        core::arch::naked_asm!("ret 4");
    }
}

port! {
    /// Clears AL, retaining all other register bits.
    #[port(address = 0x0094a5ea, owner = "shared", name = "ReturnFalse", naked)]
    pub unsafe extern "C" fn fc4p_0094a5ea() {
        core::arch::naked_asm!("xor al, al", "ret");
    }
}

port! {
    /// Returns the interface 216 bytes before this pointer.
    #[port(address = 0x0095ba6f, owner = "shared", name = "GetWindowInterface", naked)]
    pub unsafe extern "C" fn fc4p_0095ba6f() {
        core::arch::naked_asm!("lea eax, [ecx - 0xd8]", "ret");
    }
}

port! {
    /// Reads the head-relative byte into AL without widening it.
    #[port(address = 0x0096831b, owner = "shared", name = "GetHeadRelativeByte", naked)]
    pub unsafe extern "C" fn fc4p_0096831b() {
        core::arch::naked_asm!("mov al, [ecx + 0x3d]", "ret");
    }
}

port! {
    /// Returns this pointer without reading it.
    #[port(address = 0x0096d8b9, owner = "shared", name = "ReturnThis", naked)]
    pub unsafe extern "C" fn fc4p_0096d8b9() {
        core::arch::naked_asm!("mov eax, ecx", "ret");
    }
}

port! {
    /// Reads the position Y word without floating-point conversion.
    #[port(address = 0x0097ce34, owner = "shared", name = "GetPositionYBits", naked)]
    pub unsafe extern "C" fn fc4p_0097ce34() {
        core::arch::naked_asm!("mov eax, [ecx + 0x44]", "ret");
    }
}

port! {
    /// Stores the stack word at offset twelve and returns that word.
    #[port(address = 0x00989960, owner = "shared", name = "SetMessageType", naked)]
    pub unsafe extern "C" fn fc4p_00989960() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0xc], eax", "ret 4");
    }
}

port! {
    /// Stores and returns the default driver class word.
    #[port(address = 0x00990010, owner = "shared", name = "SetDefaultDriverClass", naked)]
    pub unsafe extern "C" fn fc4p_00990010() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0x64], eax", "ret 4");
    }
}

port! {
    /// Stores and returns the maximum text length word.
    #[port(address = 0x00994e71, owner = "shared", name = "SetMaximumTextLength", naked)]
    pub unsafe extern "C" fn fc4p_00994e71() {
        core::arch::naked_asm!("mov eax, [esp + 4]", "mov [ecx + 0xf8], eax", "ret 4");
    }
}

port! {
    /// Reads the window top word.
    #[port(address = 0x00994ee4, owner = "shared", name = "GetWindowTop", naked)]
    pub unsafe extern "C" fn fc4p_00994ee4() {
        core::arch::naked_asm!("mov eax, [ecx + 0xac]", "ret");
    }
}

port! {
    /// Reads the directory segment word at offset 56.
    #[port(address = 0x00999165, owner = "shared", name = "GetDirectorySegmentWord", naked)]
    pub unsafe extern "C" fn fc4p_00999165() {
        core::arch::naked_asm!("mov eax, [ecx + 0x38]", "ret");
    }
}

port! {
    /// Reads the position Z word without floating-point conversion.
    #[port(address = 0x0099b7ee, owner = "shared", name = "GetPositionZBits", naked)]
    pub unsafe extern "C" fn fc4p_0099b7ee() {
        core::arch::naked_asm!("mov eax, [ecx + 0x48]", "ret");
    }
}

port! {
    /// Reads the window left word.
    #[port(address = 0x0099bc53, owner = "shared", name = "GetWindowLeft", naked)]
    pub unsafe extern "C" fn fc4p_0099bc53() {
        core::arch::naked_asm!("mov eax, [ecx + 0xa8]", "ret");
    }
}

port! {
    /// Reads the antialiasing word.
    #[port(address = 0x0099be4c, owner = "shared", name = "GetAntialiasingWord", naked)]
    pub unsafe extern "C" fn fc4p_0099be4c() {
        core::arch::naked_asm!("mov eax, [ecx + 0x4c]", "ret");
    }
}

port! {
    /// Stores the argument byte and returns it in AL, retaining the upper bytes.
    #[port(address = 0x009b1f4b, owner = "shared", name = "SetBackgroundFillByte", naked)]
    pub unsafe extern "C" fn fc4p_009b1f4b() {
        core::arch::naked_asm!("mov al, [esp + 4]", "mov [ecx + 0x54], al", "ret 4");
    }
}

port! {
    /// Stores the two argument bytes and returns one in AL.
    #[port(address = 0x009c1772, owner = "shared", name = "SetTwoOptionBytes", naked)]
    pub unsafe extern "C" fn fc4p_009c1772() {
        core::arch::naked_asm!("mov al, [esp + 4]", "mov [ecx + 0xe], al", "mov al, [esp + 8]", "mov [ecx + 0xf], al", "mov al, 1", "ret 8");
    }
}

port! {
    /// Reads the module name pointer.
    #[port(address = 0x009c32ac, owner = "shared", name = "GetModuleNamePointer", naked)]
    pub unsafe extern "C" fn fc4p_009c32ac() {
        core::arch::naked_asm!("mov eax, [ecx + 8]", "ret");
    }
}

port! {
    /// Reads the parameter count word.
    #[port(address = 0x009cede1, owner = "shared", name = "GetParameterCount", naked)]
    pub unsafe extern "C" fn fc4p_009cede1() {
        core::arch::naked_asm!("mov eax, [ecx + 0xc]", "ret");
    }
}

port! {
    /// Reads the framework pointer.
    #[port(address = 0x009d41a1, owner = "shared", name = "GetFrameworkPointer", naked)]
    pub unsafe extern "C" fn fc4p_009d41a1() {
        core::arch::naked_asm!("mov eax, [ecx + 0x50]", "ret");
    }
}

port! {
    /// Reads the packed segment word at offset twenty.
    #[port(address = 0x009d4714, owner = "shared", name = "GetPackedSegmentWord", naked)]
    pub unsafe extern "C" fn fc4p_009d4714() {
        core::arch::naked_asm!("mov eax, [ecx + 0x14]", "ret");
    }
}

port! {
    /// Reads the script context pointer.
    #[port(address = 0x009d8564, owner = "shared", name = "GetScriptContext", naked)]
    pub unsafe extern "C" fn fc4p_009d8564() {
        core::arch::naked_asm!("mov eax, [ecx + 0x20]", "ret");
    }
}

port! {
    /// Reads the position X word without floating-point conversion.
    #[port(address = 0x009db37a, owner = "shared", name = "GetPositionXBits", naked)]
    pub unsafe extern "C" fn fc4p_009db37a() {
        core::arch::naked_asm!("mov eax, [ecx + 0x40]", "ret");
    }
}

port! {
    /// Reads the player word at offset 48.
    #[port(address = 0x009db663, owner = "shared", name = "GetPlayerPositionWord", naked)]
    pub unsafe extern "C" fn fc4p_009db663() {
        core::arch::naked_asm!("mov eax, [ecx + 0x30]", "ret");
    }
}

port! {
    /// Reads the player zoom word.
    #[port(address = 0x009db800, owner = "shared", name = "GetPlayerZoomWord", naked)]
    pub unsafe extern "C" fn fc4p_009db800() {
        core::arch::naked_asm!("mov eax, [ecx + 0x34]", "ret");
    }
}

port! {
    /// Reads the Lua pointer at offset 52.
    #[port(address = 0x00a6cab0, owner = "shared", name = "GetLuaPointer", naked)]
    pub unsafe extern "C" fn fc4p_00a6cab0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x34]", "ret");
    }
}

port! {
    /// Installs the stream table and clears the count and status byte.
    #[port(address = 0x00a6e193, owner = "shared", name = "InitializeStreamState", naked)]
    pub unsafe extern "C" fn fc4p_00a6e193() {
        core::arch::naked_asm!("mov eax, ecx", "and dword ptr [eax + 4], 0", "mov dword ptr [eax], 0xb00a5c", "mov byte ptr [eax + 8], 0", "ret");
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_008fe5ac::PORT,
    fc4p_009086ac::PORT,
    fc4p_0091559a::PORT,
    fc4p_0092973b::PORT,
    fc4p_0092b33a::PORT,
    fc4p_009378bc::PORT,
    fc4p_0093877e::PORT,
    fc4p_00949ade::PORT,
    fc4p_00949b73::PORT,
    fc4p_0094a5ea::PORT,
    fc4p_0095ba6f::PORT,
    fc4p_0096831b::PORT,
    fc4p_0096d8b9::PORT,
    fc4p_0097ce34::PORT,
    fc4p_00989960::PORT,
    fc4p_00990010::PORT,
    fc4p_00994e71::PORT,
    fc4p_00994ee4::PORT,
    fc4p_00999165::PORT,
    fc4p_0099b7ee::PORT,
    fc4p_0099bc53::PORT,
    fc4p_0099be4c::PORT,
    fc4p_009b1f4b::PORT,
    fc4p_009c1772::PORT,
    fc4p_009c32ac::PORT,
    fc4p_009cede1::PORT,
    fc4p_009d41a1::PORT,
    fc4p_009d4714::PORT,
    fc4p_009d8564::PORT,
    fc4p_009db37a::PORT,
    fc4p_009db663::PORT,
    fc4p_009db800::PORT,
    fc4p_00a6cab0::PORT,
    fc4p_00a6e193::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;

    const ENTRY_EAX: u32 = 0x89AB_CDEF;
    const ENTRY_EDX: u32 = 0x7654_3210;

    /// The call probe also checks stack cleanup and the preserved machine state.
    #[repr(C)]
    struct Probe {
        target: usize,
        eax: u32,
        ecx: u32,
        edx: u32,
        args: *const u32,
        count: u32,
        caller_pops: u32,
        saved_esp: u32,
        stack_delta: u32,
        ebx: u32,
        esi: u32,
        ebp: u32,
        flags: u32,
        cw_before: u16,
        cw_after: u16,
        sw_before: u16,
        sw_after: u16,
        mxcsr_before: u32,
        mxcsr_after: u32,
    }

    #[unsafe(naked)]
    unsafe extern "C" fn probe(_call: *mut Probe) {
        core::arch::naked_asm!(
            "push ebp",
            "push ebx",
            "push esi",
            "push edi",
            "mov edi, [esp + 20]",
            "mov [edi + 28], esp",
            "fnstcw [edi + 52]",
            "fnstsw [edi + 56]",
            "stmxcsr [edi + 60]",
            "mov ecx, [edi + 20]",
            "mov eax, [edi + 16]",
            "2:",
            "test ecx, ecx",
            "jz 3f",
            "dec ecx",
            "push dword ptr [eax + ecx * 4]",
            "jmp 2b",
            "3:",
            "mov ebx, 0x13579bdf",
            "mov esi, 0x2468ace0",
            "mov ebp, 0x11223344",
            "mov eax, [edi + 4]",
            "mov ecx, [edi + 8]",
            "mov edx, [edi + 12]",
            "call dword ptr [edi]",
            "mov [edi + 4], eax",
            "mov [edi + 8], ecx",
            "mov [edi + 12], edx",
            "mov [edi + 36], ebx",
            "mov [edi + 40], esi",
            "mov [edi + 44], ebp",
            "cmp dword ptr [edi + 24], 0",
            "je 4f",
            "mov ecx, [edi + 20]",
            "lea esp, [esp + ecx * 4]",
            "4:",
            "mov eax, esp",
            "sub eax, [edi + 28]",
            "mov [edi + 32], eax",
            "mov esp, [edi + 28]",
            "pushfd",
            "pop dword ptr [edi + 48]",
            "fnstcw [edi + 54]",
            "fnstsw [edi + 58]",
            "stmxcsr [edi + 64]",
            "pop edi",
            "pop esi",
            "pop ebx",
            "pop ebp",
            "ret",
        );
    }

    fn invoke(f: usize, this: u32, args: &[u32], eax: u32, edx: u32, cdecl: bool) -> [u32; 3] {
        let mut call = Probe {
            target: f,
            eax,
            ecx: this,
            edx,
            args: args.as_ptr(),
            count: args.len() as u32,
            caller_pops: u32::from(cdecl),
            saved_esp: 0,
            stack_delta: 0,
            ebx: 0,
            esi: 0,
            ebp: 0,
            flags: 0,
            cw_before: 0,
            cw_after: 0,
            sw_before: 0,
            sw_after: 0,
            mxcsr_before: 0,
            mxcsr_after: 0,
        };
        // SAFETY: each test supplies the export's exact convention and live object storage.
        unsafe { probe(&mut call) };
        assert_eq!(call.stack_delta, 0, "stack cleanup");
        assert_eq!((call.ebx, call.esi, call.ebp), (0x1357_9BDF, 0x2468_ACE0, 0x1122_3344));
        assert_eq!(call.flags & 0x400, 0, "direction flag");
        assert_eq!(call.cw_before, call.cw_after, "x87 control word");
        assert_eq!(call.sw_before & 0x3800, call.sw_after & 0x3800, "x87 stack top");
        assert_eq!(call.mxcsr_before, call.mxcsr_after, "MXCSR");
        [call.eax, call.ecx, call.edx]
    }

    fn run(f: usize, this: u32, args: &[u32]) -> [u32; 3] {
        invoke(f, this, args, ENTRY_EAX, ENTRY_EDX, false)
    }

    #[test]
    fn constant_byte_returns_keep_upper_bits_and_pop_exactly() {
        for (f, count, low) in [
            (fc4p_0091559a as *const () as usize, 1, 1),
            (fc4p_0092973b as *const () as usize, 3, 1),
            (fc4p_0092b33a as *const () as usize, 2, 1),
            (fc4p_009378bc as *const () as usize, 3, 0),
            (fc4p_0093877e as *const () as usize, 2, 0),
            (fc4p_00949ade as *const () as usize, 0, 1),
            (fc4p_0094a5ea as *const () as usize, 0, 0),
        ] {
            let args = [0xDEAD_BEEF, 0x1234_5678, u32::MAX];
            for eax in [0, ENTRY_EAX, u32::MAX] {
                assert_eq!(
                    invoke(f, 0xBADD_CAFE, &args[..count], eax, ENTRY_EDX, false),
                    [(eax & !255) | low, 0xBADD_CAFE, ENTRY_EDX]
                );
            }
        }
        assert_eq!(
            run(fc4p_00949b73 as *const () as usize, 0x7654, &[u32::MAX]),
            [ENTRY_EAX, 0x7654, ENTRY_EDX]
        );
    }

    #[test]
    fn word_getters_leave_the_whole_object_untouched() {
        for (f, offset) in [
            (fc4p_0097ce34 as *const () as usize, 0x44),
            (fc4p_00994ee4 as *const () as usize, 0xAC),
            (fc4p_00999165 as *const () as usize, 0x38),
            (fc4p_0099b7ee as *const () as usize, 0x48),
            (fc4p_0099bc53 as *const () as usize, 0xA8),
            (fc4p_0099be4c as *const () as usize, 0x4C),
            (fc4p_009c32ac as *const () as usize, 8),
            (fc4p_009cede1 as *const () as usize, 0xC),
            (fc4p_009d41a1 as *const () as usize, 0x50),
            (fc4p_009d4714 as *const () as usize, 0x14),
            (fc4p_009d8564 as *const () as usize, 0x20),
            (fc4p_009db37a as *const () as usize, 0x40),
            (fc4p_009db663 as *const () as usize, 0x30),
            (fc4p_009db800 as *const () as usize, 0x34),
            (fc4p_00a6cab0 as *const () as usize, 0x34),
        ] {
            for bits in [0u32, 1, u32::MAX, 0x8000_0000, 0x7FC1_2345, 0x0102_0304] {
                let mut object = [0xA5; 0x120];
                object[offset..offset + 4].copy_from_slice(&bits.to_le_bytes());
                let before = object;
                let this = object.as_mut_ptr() as u32;
                assert_eq!(run(f, this, &[]), [bits, this, ENTRY_EDX]);
                assert_eq!(object, before);
            }
        }
    }

    #[test]
    fn byte_getter_retains_entry_eax_upper_bytes() {
        for byte in [0u8, 1, 0x80, 0xFF] {
            let mut object = [0xA5; 0x50];
            object[0x3D] = byte;
            let before = object;
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_0096831b as *const () as usize, this, &[]),
                [(ENTRY_EAX & !255) | u32::from(byte), this, ENTRY_EDX]
            );
            assert_eq!(object, before);
        }
    }

    #[test]
    fn word_setters_touch_exactly_four_bytes() {
        for (f, offset) in [
            (fc4p_00989960 as *const () as usize, 0xC),
            (fc4p_00990010 as *const () as usize, 0x64),
            (fc4p_00994e71 as *const () as usize, 0xF8),
        ] {
            for word in [0u32, 1, u32::MAX, 0x8000_0000, 0x0123_4567] {
                let mut object = [0xA5; 0x120];
                let mut expected = object;
                expected[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                let this = object.as_mut_ptr() as u32;
                assert_eq!(run(f, this, &[word]), [word, this, ENTRY_EDX]);
                assert_eq!(object, expected);
            }
        }
    }

    #[test]
    fn byte_setters_store_noncanonical_bytes_and_keep_neighbors() {
        for word in [0u32, 1, 0x1234_AB80, u32::MAX] {
            let mut object = [0xA5; 0x80];
            let mut expected = object;
            expected[0x54] = word as u8;
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_009b1f4b as *const () as usize, this, &[word]),
                [(ENTRY_EAX & !255) | (word & 255), this, ENTRY_EDX]
            );
            assert_eq!(object, expected);
            expected[0xE] = word as u8;
            expected[0xF] = !word as u8;
            assert_eq!(
                run(fc4p_009c1772 as *const () as usize, this, &[word, !word]),
                [(ENTRY_EAX & !255) | 1, this, ENTRY_EDX]
            );
            assert_eq!(object, expected);
        }
    }

    #[test]
    fn constructors_write_only_initialized_fields() {
        let mut object = [0xA5; 32];
        let mut expected = object;
        let this = object.as_mut_ptr() as u32;
        expected[..4].copy_from_slice(&0xFF00_0000u32.to_le_bytes());
        assert_eq!(
            run(fc4p_008fe5ac as *const () as usize, this, &[]),
            [this, this, ENTRY_EDX]
        );
        assert_eq!(object, expected);
        expected[..12].fill(0);
        assert_eq!(
            run(fc4p_009086ac as *const () as usize, this, &[]),
            [0, this, ENTRY_EDX]
        );
        assert_eq!(object, expected);
        object.fill(0xA5);
        expected = object;
        expected[..4].copy_from_slice(&0x00B0_0A5Cu32.to_le_bytes());
        expected[4..9].fill(0);
        assert_eq!(
            run(fc4p_00a6e193 as *const () as usize, this, &[]),
            [this, this, ENTRY_EDX]
        );
        assert_eq!(object, expected);
    }

    #[test]
    fn interface_arithmetic_wraps_without_dereferencing() {
        for this in [0u32, 1, 0xD8, 0x1000, u32::MAX] {
            assert_eq!(
                run(fc4p_0095ba6f as *const () as usize, this, &[]),
                [this.wrapping_sub(0xD8), this, ENTRY_EDX]
            );
            assert_eq!(
                run(fc4p_0096d8b9 as *const () as usize, this, &[]),
                [this, this, ENTRY_EDX]
            );
        }
    }
}
