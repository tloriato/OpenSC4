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

//! Shared helpers, group 01: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::port::Port;

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00407a90, owner = "shared", name = "return_identifier_8a09f5f4", naked)]
    pub unsafe extern "C" fn fc4p_00407a90() {
        core::arch::naked_asm!("mov eax, 0x8a09f5f4", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0040ce00, owner = "shared", name = "return_identifier_6a26d049", naked)]
    pub unsafe extern "C" fn fc4p_0040ce00() {
        core::arch::naked_asm!("mov eax, 0x6a26d049", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0040d380, owner = "shared", name = "return_identifier_eac937b4", naked)]
    pub unsafe extern "C" fn fc4p_0040d380() {
        core::arch::naked_asm!("mov eax, 0xeac937b4", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00420d40, owner = "shared", name = "return_identifier_6a3848db", naked)]
    pub unsafe extern "C" fn fc4p_00420d40() {
        core::arch::naked_asm!("mov eax, 0x6a3848db", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00423000, owner = "shared", name = "return_identifier_8be3753b", naked)]
    pub unsafe extern "C" fn fc4p_00423000() {
        core::arch::naked_asm!("mov eax, 0x8be3753b", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x0042f6d0, owner = "shared", name = "return_identifier_6b70efad", naked)]
    pub unsafe extern "C" fn fc4p_0042f6d0() {
        core::arch::naked_asm!("mov eax, 0x6b70efad", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00466490, owner = "shared", name = "return_identifier_0990c38a", naked)]
    pub unsafe extern "C" fn fc4p_00466490() {
        core::arch::naked_asm!("mov eax, 0x0990c38a", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x004695a0, owner = "shared", name = "return_identifier_4990bd93", naked)]
    pub unsafe extern "C" fn fc4p_004695a0() {
        core::arch::naked_asm!("mov eax, 0x4990bd93", "ret");
    }
}

port! {
    /// Returns a fixed identifier without accessing the interface.
    #[port(address = 0x00477100, owner = "shared", name = "return_identifier_8a22bebe", naked)]
    pub unsafe extern "C" fn fc4p_00477100() {
        core::arch::naked_asm!("mov eax, 0x8a22bebe", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x78.
    #[port(address = 0x00407ab0, owner = "shared", name = "read_word_at_78", naked)]
    pub unsafe extern "C" fn fc4p_00407ab0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x78]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x74.
    #[port(address = 0x00407ac0, owner = "shared", name = "read_word_at_74", naked)]
    pub unsafe extern "C" fn fc4p_00407ac0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x74]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x7C.
    #[port(address = 0x00407ba0, owner = "shared", name = "read_word_at_7c", naked)]
    pub unsafe extern "C" fn fc4p_00407ba0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x7c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x10.
    #[port(address = 0x0040ce60, owner = "shared", name = "read_word_at_10", naked)]
    pub unsafe extern "C" fn fc4p_0040ce60() {
        core::arch::naked_asm!("mov eax, [ecx + 0x10]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0xE0.
    #[port(address = 0x00423650, owner = "shared", name = "read_word_at_e0", naked)]
    pub unsafe extern "C" fn fc4p_00423650() {
        core::arch::naked_asm!("mov eax, [ecx + 0xe0]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x110.
    #[port(address = 0x00423700, owner = "shared", name = "read_word_at_110", naked)]
    pub unsafe extern "C" fn fc4p_00423700() {
        core::arch::naked_asm!("mov eax, [ecx + 0x110]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x12C.
    #[port(address = 0x00423710, owner = "shared", name = "read_word_at_12c", naked)]
    pub unsafe extern "C" fn fc4p_00423710() {
        core::arch::naked_asm!("mov eax, [ecx + 0x12c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x14C.
    #[port(address = 0x00423750, owner = "shared", name = "read_word_at_14c", naked)]
    pub unsafe extern "C" fn fc4p_00423750() {
        core::arch::naked_asm!("mov eax, [ecx + 0x14c]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x198.
    #[port(address = 0x00423850, owner = "shared", name = "read_word_at_198", naked)]
    pub unsafe extern "C" fn fc4p_00423850() {
        core::arch::naked_asm!("mov eax, [ecx + 0x198]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x150.
    #[port(address = 0x0043d230, owner = "shared", name = "read_word_at_150", naked)]
    pub unsafe extern "C" fn fc4p_0043d230() {
        core::arch::naked_asm!("mov eax, [ecx + 0x150]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x1C0.
    #[port(address = 0x00459880, owner = "shared", name = "read_word_at_1c0", naked)]
    pub unsafe extern "C" fn fc4p_00459880() {
        core::arch::naked_asm!("mov eax, [ecx + 0x1c0]", "ret");
    }
}

port! {
    /// Returns the complete word at interface offset 0x8.
    #[port(address = 0x004777e0, owner = "shared", name = "read_word_at_8", naked)]
    pub unsafe extern "C" fn fc4p_004777e0() {
        core::arch::naked_asm!("mov eax, [ecx + 0x8]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x50, without reading it.
    #[port(address = 0x00407bb0, owner = "shared", name = "address_of_member_at_50", naked)]
    pub unsafe extern "C" fn fc4p_00407bb0() {
        core::arch::naked_asm!("lea eax, [ecx + 0x50]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x24, without reading it.
    #[port(address = 0x00477810, owner = "shared", name = "address_of_member_at_24", naked)]
    pub unsafe extern "C" fn fc4p_00477810() {
        core::arch::naked_asm!("lea eax, [ecx + 0x24]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x90, without reading it.
    #[port(address = 0x00477890, owner = "shared", name = "address_of_member_at_90", naked)]
    pub unsafe extern "C" fn fc4p_00477890() {
        core::arch::naked_asm!("lea eax, [ecx + 0x90]", "ret");
    }
}

port! {
    /// Returns the address at interface offset 0x9C, without reading it.
    #[port(address = 0x004778a0, owner = "shared", name = "address_of_member_at_9c", naked)]
    pub unsafe extern "C" fn fc4p_004778a0() {
        core::arch::naked_asm!("lea eax, [ecx + 0x9c]", "ret");
    }
}

port! {
    /// Loads the float at offset 0x38 onto the caller's x87 stack.
    #[port(address = 0x00477860, owner = "shared", name = "load_float_at_38", naked)]
    pub unsafe extern "C" fn fc4p_00477860() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0x38]", "ret");
    }
}

port! {
    /// Restores the value's vtable word and preserves the entry registers.
    #[port(address = 0x00489660, owner = "shared", name = "restore_value_vtable", naked)]
    pub unsafe extern "C" fn fc4p_00489660() {
        core::arch::naked_asm!("mov dword ptr [ecx], 0x00a8ec1c", "ret");
    }
}

port! {
    /// Forwards the original arguments to virtual slot 0x24.
    #[port(address = 0x00420130, owner = "shared", name = "forward_virtual_slot_24", naked)]
    pub unsafe extern "C" fn fc4p_00420130() {
        core::arch::naked_asm!("mov eax, [ecx]", "jmp dword ptr [eax + 0x24]");
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_advice_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x00407bc0;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_advice_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00407bc0",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00407bc0.
    #[port(address = 0x00408720, owner = "shared", name = "query_advice_primary", naked)]
    pub unsafe extern "C" fn fc4p_00408720() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00407bc0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_advice_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn delete_advice_primary_target(this: *mut u8, arg0: u32) -> u64 = "thiscall" 0x0043ec10;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn delete_advice_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0043ec10",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0043ec10.
    #[port(address = 0x00408730, owner = "shared", name = "delete_advice_primary", naked)]
    pub unsafe extern "C" fn fc4p_00408730() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0043ec10");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym delete_advice_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_advisor_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0040d430;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0040d430",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -8 bytes and forwards to 0x0040d430.
    #[port(address = 0x0040f370, owner = "shared", name = "query_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0040f370() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x0040d430");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym query_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_advisor_reference_target(this: *mut u8) -> u64 = "thiscall" 0x00968b60;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_advisor_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00968b60",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 20 bytes and forwards to 0x00968b60.
    #[port(address = 0x004186e0, owner = "shared", name = "retain_advisor_reference", naked)]
    pub unsafe extern "C" fn fc4p_004186e0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 20", "jmp dword ptr [2f]", "2: .long 0x00968b60");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 20", "jmp {}", sym retain_advisor_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_advisor_primary_target(this: *mut u8) -> u64 = "thiscall" 0x004186e0;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x004186e0",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -16 bytes and forwards to 0x004186e0.
    #[port(address = 0x0041a260, owner = "shared", name = "retain_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0041a260() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 16", "jmp dword ptr [2f]", "2: .long 0x004186e0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 16", "jmp {}", sym retain_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_advisor_primary_target(this: *mut u8) -> u64 = "thiscall" 0x00449310;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_advisor_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00449310",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -16 bytes and forwards to 0x00449310.
    #[port(address = 0x0041a270, owner = "shared", name = "release_advisor_primary", naked)]
    pub unsafe extern "C" fn fc4p_0041a270() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 16", "jmp dword ptr [2f]", "2: .long 0x00449310");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 16", "jmp {}", sym release_advisor_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn retain_overlay_reference_target(this: *mut u8) -> u64 = "thiscall" 0x00968b60;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn retain_overlay_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00968b60",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 8 bytes and forwards to 0x00968b60.
    #[port(address = 0x004302a0, owner = "shared", name = "retain_overlay_reference", naked)]
    pub unsafe extern "C" fn fc4p_004302a0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 8", "jmp dword ptr [2f]", "2: .long 0x00968b60");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 8", "jmp {}", sym retain_overlay_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_overlay_reference_target(this: *mut u8) -> u64 = "thiscall" 0x0090d997;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_overlay_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0090d997",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 8 bytes and forwards to 0x0090d997.
    #[port(address = 0x004302b0, owner = "shared", name = "release_overlay_reference", naked)]
    pub unsafe extern "C" fn fc4p_004302b0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 8", "jmp dword ptr [2f]", "2: .long 0x0090d997");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 8", "jmp {}", sym release_overlay_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn release_advisor_reference_target(this: *mut u8) -> u64 = "thiscall" 0x0090d997;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn release_advisor_reference_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0090d997",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by 20 bytes and forwards to 0x0090d997.
    #[port(address = 0x00449310, owner = "shared", name = "release_advisor_reference", naked)]
    pub unsafe extern "C" fn fc4p_00449310() {
        #[cfg(not(test))]
        core::arch::naked_asm!("add ecx, 20", "jmp dword ptr [2f]", "2: .long 0x0090d997");
        #[cfg(test)]
        core::arch::naked_asm!("add ecx, 20", "jmp {}", sym release_advisor_reference_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_tool_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0045e760;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_tool_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0045e760",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0045e760.
    #[port(address = 0x0045fcd0, owner = "shared", name = "query_tool_primary", naked)]
    pub unsafe extern "C" fn fc4p_0045fcd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0045e760");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_tool_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_terrain_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0092d091;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_terrain_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0092d091",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0092d091.
    #[port(address = 0x00472fd0, owner = "shared", name = "query_terrain_primary", naked)]
    pub unsafe extern "C" fn fc4p_00472fd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0092d091");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_terrain_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn dispatch_terrain_primary_target(this: *mut u8, arg0: u32) -> u64 = "thiscall" 0x0092e040;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn dispatch_terrain_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0092e040",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0092e040.
    #[port(address = 0x00473000, owner = "shared", name = "dispatch_terrain_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473000() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0092e040");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym dispatch_terrain_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_brush_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x0087931b;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_brush_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x0087931b",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x0087931b.
    #[port(address = 0x00473050, owner = "shared", name = "query_brush_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473050() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x0087931b");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_brush_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn dispatch_brush_primary_target(this: *mut u8) -> u64 = "thiscall" 0x00879383;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn dispatch_brush_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00879383",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00879383.
    #[port(address = 0x00473070, owner = "shared", name = "dispatch_brush_primary", naked)]
    pub unsafe extern "C" fn fc4p_00473070() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00879383");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym dispatch_brush_primary_test_target);
    }
}

game_fn! {
    /// Tail-call destination; the export forwards every original register and stack word.
    #[allow(dead_code)]
    fn query_terrain_system_primary_target(this: *mut u8, arg0: u32, arg1: u32) -> u64 = "thiscall" 0x00477130;
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn query_terrain_system_primary_test_target() {
    // Preserve the entry registers while resolving the test's fake, then tail-call it.
    core::arch::naked_asm!("push eax", "push ecx", "push edx", "push 0x00477130",
        "call {}", "add esp, 4", "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
        sym resolve_tail_target);
}

port! {
    /// Adjusts the interface by -4 bytes and forwards to 0x00477130.
    #[port(address = 0x00477210, owner = "shared", name = "query_terrain_system_primary", naked)]
    pub unsafe extern "C" fn fc4p_00477210() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x00477130");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym query_terrain_system_primary_test_target);
    }
}

#[cfg(test)]
extern "C" fn resolve_tail_target(address: u32) -> usize {
    crate::testing::resolve(address)
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00407a90::PORT,
    fc4p_00407ab0::PORT,
    fc4p_00407ac0::PORT,
    fc4p_00407ba0::PORT,
    fc4p_00407bb0::PORT,
    fc4p_00408720::PORT,
    fc4p_00408730::PORT,
    fc4p_0040ce00::PORT,
    fc4p_0040ce60::PORT,
    fc4p_0040d380::PORT,
    fc4p_0040f370::PORT,
    fc4p_004186e0::PORT,
    fc4p_0041a260::PORT,
    fc4p_0041a270::PORT,
    fc4p_00420130::PORT,
    fc4p_00420d40::PORT,
    fc4p_00423000::PORT,
    fc4p_00423650::PORT,
    fc4p_00423700::PORT,
    fc4p_00423710::PORT,
    fc4p_00423750::PORT,
    fc4p_00423850::PORT,
    fc4p_0042f6d0::PORT,
    fc4p_004302a0::PORT,
    fc4p_004302b0::PORT,
    fc4p_0043d230::PORT,
    fc4p_00449310::PORT,
    fc4p_00459880::PORT,
    fc4p_0045fcd0::PORT,
    fc4p_00466490::PORT,
    fc4p_004695a0::PORT,
    fc4p_00472fd0::PORT,
    fc4p_00473000::PORT,
    fc4p_00473050::PORT,
    fc4p_00473070::PORT,
    fc4p_00477100::PORT,
    fc4p_00477210::PORT,
    fc4p_004777e0::PORT,
    fc4p_00477810::PORT,
    fc4p_00477860::PORT,
    fc4p_00477890::PORT,
    fc4p_004778a0::PORT,
    fc4p_00489660::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{call_thiscall_with_eax, fake, GameFpu};

    #[test]
    fn constants_return_full_words_and_preserve_edx() {
        let entries: &[(unsafe extern "C" fn(), u32)] = &[
            (fc4p_00407a90, 0x8a09f5f4),
            (fc4p_0040ce00, 0x6a26d049),
            (fc4p_0040d380, 0xeac937b4),
            (fc4p_00420d40, 0x6a3848db),
            (fc4p_00423000, 0x8be3753b),
            (fc4p_0042f6d0, 0x6b70efad),
            (fc4p_00466490, 0x0990c38a),
            (fc4p_004695a0, 0x4990bd93),
            (fc4p_00477100, 0x8a22bebe),
        ];
        for &(entry, expected) in entries {
            let mut call = RawCall::new(Conv::Thiscall, entry as usize, 0, &[]);
            call.eax = 0xA5A5_5A5A;
            call.edx = 0x1234_5678;
            unsafe { call_raw(&mut call) };
            assert_eq!((call.eax, call.edx), (expected, 0x1234_5678));
        }
    }

    #[test]
    fn getters_read_full_words_without_writes() {
        let mut bytes = [0xA5u8; 0x200];
        let entries: &[(unsafe extern "C" fn(), usize)] = &[
            (fc4p_00407ab0, 0x78),
            (fc4p_00407ac0, 0x74),
            (fc4p_00407ba0, 0x7c),
            (fc4p_0040ce60, 0x10),
            (fc4p_00423650, 0xe0),
            (fc4p_00423700, 0x110),
            (fc4p_00423710, 0x12c),
            (fc4p_00423750, 0x14c),
            (fc4p_00423850, 0x198),
            (fc4p_0043d230, 0x150),
            (fc4p_00459880, 0x1c0),
            (fc4p_004777e0, 0x8),
        ];
        for &(entry, offset) in entries {
            for word in [0u32, 1, u32::MAX, 0x8040_FF81] {
                bytes[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                let before = bytes;
                let result = unsafe { call_thiscall_with_eax(entry as usize, bytes.as_mut_ptr(), &[], 0x5A5A_A5A5) };
                assert_eq!(result, (word, 0));
                assert_eq!(bytes, before);
            }
        }
    }

    #[test]
    fn member_addresses_accept_null_and_wrapping_interfaces() {
        let entries: &[(unsafe extern "C" fn(), u32)] = &[
            (fc4p_00407bb0, 0x50),
            (fc4p_00477810, 0x24),
            (fc4p_00477890, 0x90),
            (fc4p_004778a0, 0x9c),
        ];
        for &(entry, offset) in entries {
            for base in [0u32, 0x1234_5678, u32::MAX - 16] {
                let result = unsafe { call_thiscall_with_eax(entry as usize, base as *mut u8, &[], 0) };
                assert_eq!(result, (base.wrapping_add(offset), 0));
            }
        }
    }

    #[test]
    fn float_getter_keeps_integer_registers_and_existing_x87_value() {
        let _fpu = GameFpu::new();
        let mut object = [0u32; 16];
        for bits in [
            0,
            0x8000_0000,
            0x3FC0_0000,
            0x7F80_0000,
            0xFF80_0000,
            0x7FC0_1234,
            0x7F80_1234,
        ] {
            object[0x38 / 4] = bits;
            let mut call = RawCall::new(
                Conv::Thiscall,
                fc4p_00477860 as *const () as usize,
                object.as_mut_ptr() as u32,
                &[],
            );
            call.eax = 0xABCD_1234;
            call.edx = 0x9876_5432;
            call.take_st0 = 1;
            let sentinel = 123.25f64;
            let mut restored = 0f64;
            unsafe {
                core::arch::asm!("fld qword ptr [{p}]", p = in(reg) &sentinel, out("st(0)") _);
            }
            unsafe { call_raw(&mut call) };
            unsafe {
                core::arch::asm!("fstp qword ptr [{p}]", p = in(reg) &mut restored, out("st(0)") _);
            }
            assert_eq!(restored, sentinel);
            assert_eq!((call.eax, call.edx), (0xABCD_1234, 0x9876_5432));
            assert_eq!(
                call.st0.to_f32().to_bits(),
                if bits & 0x7FFF_FFFF > 0x7F80_0000 {
                    bits | 0x0040_0000
                } else {
                    bits
                }
            );
        }
    }

    #[test]
    fn vtable_restore_writes_only_the_first_word_and_keeps_eax() {
        let mut object = [0xDEAD_BEEFu32; 4];
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_00489660 as *const () as usize,
                object.as_mut_ptr().cast(),
                &[],
                0x1234_ABCD,
            )
        };
        assert_eq!(result, (0x1234_ABCD, 0));
        assert_eq!(object, [0x00A8_EC1C, 0xDEAD_BEEF, 0xDEAD_BEEF, 0xDEAD_BEEF]);
    }

    thread_local! {
        static FORWARDED: std::cell::RefCell<Vec<(usize, Vec<u32>)>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    extern "thiscall" fn target0(this: *mut u8) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![])));
        0xABCD_1234_5678_90EF
    }
    extern "thiscall" fn target1(this: *mut u8, a: u32) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![a])));
        0xABCD_1234_5678_90EF
    }
    extern "thiscall" fn target2(this: *mut u8, a: u32, b: u32) -> u64 {
        FORWARDED.with(|v| v.borrow_mut().push((this as usize, vec![a, b])));
        0xABCD_1234_5678_90EF
    }

    #[test]
    fn interface_thunks_forward_arguments_adjust_this_and_return_both_words() {
        let entries: &[(unsafe extern "C" fn(), u32, i32, usize)] = &[
            (fc4p_00408720, 0x00407bc0, -4, 2),
            (fc4p_00408730, 0x0043ec10, -4, 1),
            (fc4p_0040f370, 0x0040d430, -8, 2),
            (fc4p_004186e0, 0x00968b60, 20, 0),
            (fc4p_0041a260, 0x004186e0, -16, 0),
            (fc4p_0041a270, 0x00449310, -16, 0),
            (fc4p_004302a0, 0x00968b60, 8, 0),
            (fc4p_004302b0, 0x0090d997, 8, 0),
            (fc4p_00449310, 0x0090d997, 20, 0),
            (fc4p_0045fcd0, 0x0045e760, -4, 2),
            (fc4p_00472fd0, 0x0092d091, -4, 2),
            (fc4p_00473000, 0x0092e040, -4, 1),
            (fc4p_00473050, 0x0087931b, -4, 2),
            (fc4p_00473070, 0x00879383, -4, 0),
            (fc4p_00477210, 0x00477130, -4, 2),
        ];
        for &(entry, target, offset, count) in entries {
            let f = [
                target0 as *const () as usize,
                target1 as *const () as usize,
                target2 as *const () as usize,
            ][count];
            let _fake = fake(target, f);
            let args = [0x1122_3344, 0xAABB_CCDD];
            for base in [0u32, 0x1234_5678, u32::MAX - 4] {
                FORWARDED.with(|v| v.borrow_mut().clear());
                let result = unsafe { call_thiscall_with_eax(entry as usize, base as *mut u8, &args[..count], 0) };
                assert_eq!(result, (0x5678_90EF, 0xABCD_1234));
                FORWARDED.with(|v| {
                    assert_eq!(
                        *v.borrow(),
                        [(base.wrapping_add(offset as u32) as usize, args[..count].to_vec())]
                    )
                });
            }
        }
    }

    #[test]
    fn virtual_forwarding_keeps_all_stack_arguments() {
        let mut table = [0usize; 10];
        table[9] = target2 as *const () as usize;
        let mut object = [table.as_ptr() as usize, 0x1234_5678];
        FORWARDED.with(|v| v.borrow_mut().clear());
        let result = unsafe {
            call_thiscall_with_eax(
                fc4p_00420130 as *const () as usize,
                object.as_mut_ptr().cast(),
                &[17, 99],
                0,
            )
        };
        assert_eq!(result, (0x5678_90EF, 0xABCD_1234));
        FORWARDED.with(|v| assert_eq!(*v.borrow(), [(object.as_ptr() as usize, vec![17, 99])]));
        assert_eq!(object[1], 0x1234_5678);
    }
}
