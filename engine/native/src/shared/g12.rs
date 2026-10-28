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

/// The entry saves the three volatile registers immediately below the original return address.
/// Rust bodies edit their return values here so that partial results and untouched registers survive.
#[repr(C)]
struct Registers {
    eax: u32,
    ecx: u32,
    edx: u32,
}

impl Registers {
    unsafe fn argument(&self, index: usize) -> u32 {
        // SAFETY: the entry's frame is followed by the original return address and stack arguments.
        unsafe { (self as *const Self).cast::<u32>().add(4 + index).read_unaligned() }
    }

    unsafe fn virtual_call(&mut self, object: u32, slot: usize, args: &[u32]) {
        #[repr(C)]
        struct Call {
            target: usize,
            eax: u32,
            ecx: u32,
            edx: u32,
            args: *const u32,
            count: u32,
        }
        let mut call = Call {
            // SAFETY: the enclosing port supplies a live object and its original slot.
            target: unsafe { crate::abi::vslot(object as *mut u8, slot) },
            eax: self.eax,
            ecx: object,
            edx: self.edx,
            args: args.as_ptr(),
            count: args.len() as u32,
        };
        // Capture ECX too: several callers reuse the value left by these small virtual methods.
        // SAFETY: all slots used here are thiscall and remove exactly the supplied stack words.
        unsafe {
            core::arch::asm!(
                "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
                "2:", "test ecx, ecx", "jz 3f", "dec ecx",
                "push dword ptr [eax + ecx * 4]", "jmp 2b",
                "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
                "call dword ptr [edi]",
                "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                in("edi") &mut call, clobber_abi("C"),
            );
        }
        self.eax = call.eax;
        self.ecx = call.ecx;
        self.edx = call.edx;
    }
}

/// The small entry only marshals registers; the function's behavior is implemented by its Rust body.
macro_rules! register_port {
    ($address:literal, $export:ident, $body:ident, $name:literal, $pop:literal, |$r:ident| $code:block) => {
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
                    "pop eax", "pop ecx", "pop edx", "ret {pop}",
                    body = sym $body, pop = const $pop,
                );
            }
        }
        unsafe extern "thiscall" fn $body(frame: *mut Registers) {
            // SAFETY: the entry supplies its private, live register frame.
            let $r = unsafe { &mut *frame };
            $code
        }
    };
}

unsafe fn word(object: u32, offset: usize) -> u32 {
    // SAFETY: each port supplies its original object's accessible field.
    unsafe {
        (object as *const u8)
            .wrapping_add(offset)
            .cast::<u32>()
            .read_unaligned()
    }
}

unsafe fn put_word(object: u32, offset: usize, value: u32) {
    // SAFETY: each port supplies its original object's writable field.
    unsafe {
        (object as *mut u8)
            .wrapping_add(offset)
            .cast::<u32>()
            .write_unaligned(value)
    }
}

unsafe fn add_reference(r: &mut Registers, offset: usize) {
    // SAFETY: the reference count is a writable word of the live object.
    unsafe {
        r.eax = word(r.ecx, offset).wrapping_add(1);
        put_word(r.ecx, offset, r.eax);
    }
}

unsafe fn release_reference(r: &mut Registers, offset: usize, signed: bool, adjustment: u32, slot: usize) {
    // SAFETY: the reference word and deleting-destructor slot belong to the live object.
    unsafe {
        r.eax = word(r.ecx, offset);
        if if signed { (r.eax as i32) > 1 } else { r.eax > 1 } {
            r.eax = r.eax.wrapping_sub(1);
            put_word(r.ecx, offset, r.eax);
        } else {
            let object = r.ecx.wrapping_add(adjustment);
            r.eax = word(object, 0);
            r.virtual_call(object, slot, &[1]);
            r.eax = 0;
        }
    }
}

unsafe fn copy_key_to_output(r: &mut Registers, offset: usize) {
    // Read and write each word in sequence, including when output overlaps the object's fields.
    // SAFETY: the game supplies three readable key words and a three-word output.
    unsafe {
        let object = r.ecx;
        r.edx = word(object, offset);
        r.eax = r.argument(0);
        put_word(r.eax, 0, r.edx);
        r.edx = word(object, offset + 4);
        put_word(r.eax, 4, r.edx);
        r.ecx = word(object, offset + 8);
        put_word(r.eax, 8, r.ecx);
    }
}

game_data! { static directory_type_table: [u32; 80] = 0x00B1_69F8; }

register_port!(
    0x009100de,
    fc4p_009100de,
    is_thai_character,
    "IsThaiCharacter",
    0,
    |r| {
        // SAFETY: one cdecl stack word; the comparison uses all 32 bits.
        r.eax = u32::from((0xE00..=0xE7F).contains(&unsafe { r.argument(0) }));
    }
);

register_port!(
    0x009103b0,
    fc4p_009103b0,
    is_utf8_leading_byte,
    "IsUtf8LeadingByte",
    0,
    |r| {
        // SAFETY: one cdecl stack word; only its low byte is tested.
        r.eax = u32::from((0xC0..=0xFD).contains(&(unsafe { r.argument(0) } as u8)));
    }
);

register_port!(
    0x00920bf0,
    fc4p_00920bf0,
    release_cheat_manager,
    "ReleaseCheatManager",
    0,
    |r| {
        // SAFETY: unsigned reference count and the secondary interface's deleting destructor.
        unsafe { release_reference(r, 0x20, false, 8, 0xC) };
    }
);

register_port!(
    0x009334b1,
    fc4p_009334b1,
    add_reference_when_enabled,
    "AddReferenceWhenEnabled",
    0,
    |r| {
        // SAFETY: the flag byte and reference word belong to this object.
        unsafe {
            if *((r.ecx as *const u8).add(9)) != 0 {
                add_reference(r, 0xC);
            } else {
                r.eax = 1;
            }
        }
    }
);

register_port!(
    0x009334c2,
    fc4p_009334c2,
    release_reference_when_enabled,
    "ReleaseReferenceWhenEnabled",
    0,
    |r| {
        // SAFETY: the flag byte, reference word and deleting destructor belong to this object.
        unsafe {
            if *((r.ecx as *const u8).add(9)) != 0 {
                release_reference(r, 0xC, false, 0, 0x20);
            } else {
                r.eax = 1;
            }
        }
    }
);

register_port!(
    0x009377c9,
    fc4p_009377c9,
    directory_type_at_index,
    "DirectoryTypeAtIndex",
    0,
    |r| {
        // SAFETY: the table contains 80 words; out-of-range indices do not access it.
        unsafe {
            let index = r.argument(0);
            r.eax = if index < 80 {
                directory_type_table()
                    .cast::<u32>()
                    .add(index as usize)
                    .read_unaligned()
            } else {
                0
            };
        }
    }
);

register_port!(
    0x00937ef1,
    fc4p_00937ef1,
    copy_record_interface,
    "CopyRecordInterface",
    4,
    |r| {
        // SAFETY: an optional two-word output and the original object's second word.
        unsafe {
            r.eax = r.argument(0);
            if r.eax != 0 {
                put_word(r.eax, 0, 0x00AD_25FC);
                r.ecx = word(r.ecx, 4);
                put_word(r.eax, 4, r.ecx);
            }
        }
    }
);

register_port!(
    0x00943917,
    fc4p_00943917,
    release_signed_reference,
    "ReleaseSignedReference",
    0,
    |r| {
        // SAFETY: this object's signed count and deleting-destructor slot.
        unsafe { release_reference(r, 4, true, 0, 0x44) };
    }
);

register_port!(
    0x00945942,
    fc4p_00945942,
    add_cell_player_reference,
    "AddCellPlayerReference",
    0,
    |r| {
        // SAFETY: the cell player's reference word.
        unsafe { add_reference(r, 0x14) };
    }
);
register_port!(
    0x0094bc3f,
    fc4p_0094bc3f,
    add_cheat_manager_reference,
    "AddCheatManagerReference",
    0,
    |r| {
        // SAFETY: the cheat manager's reference word.
        unsafe { add_reference(r, 0x20) };
    }
);
register_port!(
    0x0094c4b1,
    fc4p_0094c4b1,
    add_network_cache_reference,
    "AddNetworkCacheReference",
    0,
    |r| {
        // SAFETY: the network cache's reference word.
        unsafe { add_reference(r, 8) };
    }
);
register_port!(
    0x0094d34f,
    fc4p_0094d34f,
    add_file_browser_reference,
    "AddFileBrowserReference",
    0,
    |r| {
        // SAFETY: the file browser's reference word.
        unsafe { add_reference(r, 0x10) };
    }
);

register_port!(
    0x00968303,
    fc4p_00968303,
    get_listener_interface,
    "GetListenerInterface",
    0,
    |r| {
        let object = r.ecx;
        r.ecx = object.wrapping_add(0xC);
        r.eax = if object == 0 { 0 } else { r.ecx };
    }
);

register_port!(0x0096d895, fc4p_0096d895, keys_equal, "KeysEqual", 4, |r| {
    // SAFETY: both pointers cover the three key words read before the first mismatch.
    unsafe {
        let other = r.argument(0);
        r.edx = word(r.ecx, 0);
        r.eax = 0;
        if r.edx == word(other, 0) {
            r.edx = word(r.ecx, 4);
            if r.edx == word(other, 4) {
                r.ecx = word(r.ecx, 8);
                r.eax = u32::from(r.ecx == word(other, 8));
            }
        }
    }
});

register_port!(
    0x0096e773,
    fc4p_0096e773,
    initialize_retained_pointer,
    "InitializeRetainedPointer",
    4,
    |r| {
        // SAFETY: the output word and optional retained interface are live game objects.
        unsafe {
            let output = r.ecx;
            r.ecx = r.argument(0);
            put_word(output, 0, r.ecx);
            if r.ecx != 0 {
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 4, &[]);
            }
            r.eax = output;
        }
    }
);

register_port!(0x0096e957, fc4p_0096e957, has_third_word, "HasThirdWord", 0, |r| {
    // SAFETY: the third word belongs to this object.
    r.eax = u32::from(unsafe { word(r.ecx, 8) } != 0);
});

register_port!(
    0x009745b0,
    fc4p_009745b0,
    release_unsigned_reference,
    "ReleaseUnsignedReference",
    0,
    |r| {
        // SAFETY: this object's unsigned count and deleting-destructor slot.
        unsafe { release_reference(r, 4, false, 0, 0x10) };
    }
);

register_port!(
    0x009753ea,
    fc4p_009753ea,
    copy_key_at_twelve,
    "CopyKeyAtTwelve",
    4,
    |r| {
        // SAFETY: the record key and three-word output.
        unsafe { copy_key_to_output(r, 0xC) };
    }
);

register_port!(0x00975402, fc4p_00975402, set_key_at_twelve, "SetKeyAtTwelve", 4, |r| {
    // SAFETY: the source key and writable record key; each word is read after the preceding write.
    unsafe {
        let source = r.argument(0);
        r.edx = word(source, 0);
        put_word(r.ecx, 0xC, r.edx);
        r.edx = word(source, 4);
        put_word(r.ecx, 0x10, r.edx);
        r.eax = word(source, 8);
        put_word(r.ecx, 0x14, r.eax);
        r.eax = (r.eax & !255) | 1;
    }
});

register_port!(0x00975666, fc4p_00975666, copy_key_at_eight, "CopyKeyAtEight", 4, |r| {
    // SAFETY: the record key and three-word output.
    unsafe { copy_key_to_output(r, 8) };
});

register_port!(
    0x009779c9,
    fc4p_009779c9,
    retain_second_pointer,
    "RetainSecondPointer",
    0,
    |r| {
        // SAFETY: the holder and optional retained interface. The pointer is reloaded after AddRef.
        unsafe {
            let holder = r.ecx;
            if word(holder, 4) != 0 {
                r.ecx = word(holder, 4);
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 4, &[]);
            }
            r.eax = word(holder, 4);
        }
    }
);

register_port!(0x00999055, fc4p_00999055, any_flags_set, "AnyFlagsSet", 4, |r| {
    // SAFETY: the flag word and one stack mask.
    r.eax = u32::from(unsafe { word(r.ecx, 4) & r.argument(0) } != 0);
});

register_port!(
    0x009a5faa,
    fc4p_009a5faa,
    initialize_window_once,
    "InitializeWindowOnce",
    0,
    |r| {
        // SAFETY: the status byte and secondary interface's initialization slot belong to this window.
        unsafe {
            r.eax = r.ecx.wrapping_add(0xEC);
            if *(r.eax as *const u8) == 0 {
                *(r.eax as *mut u8) = 1;
                r.ecx = r.ecx.wrapping_add(4);
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 0x170, &[]);
            }
        }
    }
);

register_port!(
    0x009c3eb7,
    fc4p_009c3eb7,
    get_secondary_window_interface,
    "GetSecondaryWindowInterface",
    0,
    |r| {
        let base = r.ecx.wrapping_sub(0x124);
        r.ecx = r.ecx.wrapping_sub(0x4C);
        r.eax = if base == 0 { 0 } else { r.ecx };
    }
);

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
    fc4p_009100de::PORT,
    fc4p_009103b0::PORT,
    fc4p_0091559a::PORT,
    fc4p_00920bf0::PORT,
    fc4p_0092973b::PORT,
    fc4p_0092b33a::PORT,
    fc4p_009334b1::PORT,
    fc4p_009334c2::PORT,
    fc4p_009377c9::PORT,
    fc4p_009378bc::PORT,
    fc4p_00937ef1::PORT,
    fc4p_0093877e::PORT,
    fc4p_00943917::PORT,
    fc4p_00945942::PORT,
    fc4p_00949ade::PORT,
    fc4p_00949b73::PORT,
    fc4p_0094a5ea::PORT,
    fc4p_0094bc3f::PORT,
    fc4p_0094c4b1::PORT,
    fc4p_0094d34f::PORT,
    fc4p_0095ba6f::PORT,
    fc4p_00968303::PORT,
    fc4p_0096831b::PORT,
    fc4p_0096d895::PORT,
    fc4p_0096d8b9::PORT,
    fc4p_0096e773::PORT,
    fc4p_0096e957::PORT,
    fc4p_009745b0::PORT,
    fc4p_009753ea::PORT,
    fc4p_00975402::PORT,
    fc4p_00975666::PORT,
    fc4p_009779c9::PORT,
    fc4p_0097ce34::PORT,
    fc4p_00989960::PORT,
    fc4p_00990010::PORT,
    fc4p_00994e71::PORT,
    fc4p_00994ee4::PORT,
    fc4p_00999055::PORT,
    fc4p_00999165::PORT,
    fc4p_0099b7ee::PORT,
    fc4p_0099bc53::PORT,
    fc4p_0099be4c::PORT,
    fc4p_009a5faa::PORT,
    fc4p_009b1f4b::PORT,
    fc4p_009c1772::PORT,
    fc4p_009c32ac::PORT,
    fc4p_009c3eb7::PORT,
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
    use std::cell::RefCell;

    const ENTRY_EAX: u32 = 0x89AB_CDEF;
    const ENTRY_EDX: u32 = 0x7654_3210;
    const RESULT_EAX: u32 = 0xA1B2_C3D4;
    const RESULT_ECX: u32 = 0x4321_ABCD;
    const RESULT_EDX: u32 = 0xD4C3_B2A1;

    #[derive(Debug, PartialEq)]
    struct VirtualCall {
        this: u32,
        eax: u32,
        edx: u32,
        argument: Option<u32>,
    }

    #[derive(Default)]
    struct State {
        calls: Vec<VirtualCall>,
        watch: Option<(u32, usize)>,
        snapshots: Vec<Vec<u8>>,
        mutation: Option<(u32, usize, u32)>,
    }

    thread_local! { static STATE: RefCell<State> = RefCell::new(State::default()); }

    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }

    fn calls() -> Vec<VirtualCall> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().calls))
    }

    unsafe fn observe(frame: *mut Registers, has_argument: bool) {
        // SAFETY: the fake entry supplies its saved registers and original argument words.
        unsafe {
            let r = &*frame;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.calls.push(VirtualCall {
                    this: r.ecx,
                    eax: r.eax,
                    edx: r.edx,
                    argument: if has_argument { Some(r.argument(0)) } else { None },
                });
                if let Some((p, n)) = s.watch {
                    s.snapshots
                        .push(core::slice::from_raw_parts(p as *const u8, n).to_vec());
                }
                if let Some((p, offset, value)) = s.mutation {
                    put_word(p, offset, value);
                }
            });
        }
    }

    unsafe extern "thiscall" fn observe_zero(frame: *mut Registers) {
        // SAFETY: the fake's register frame, with no stack arguments.
        unsafe { observe(frame, false) };
    }

    unsafe extern "thiscall" fn observe_one(frame: *mut Registers) {
        // SAFETY: the fake's register frame, with one stack argument.
        unsafe { observe(frame, true) };
    }

    #[unsafe(naked)]
    unsafe extern "C" fn fake_zero() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}", "add esp, 12",
            "mov eax, 0xa1b2c3d4", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret",
            sym observe_zero,
        );
    }

    #[unsafe(naked)]
    unsafe extern "C" fn fake_one() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}", "add esp, 12",
            "mov eax, 0xa1b2c3d4", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret 4",
            sym observe_one,
        );
    }

    fn expected_call(this: u32, eax: u32, argument: Option<u32>) -> VirtualCall {
        VirtualCall {
            this,
            eax,
            edx: ENTRY_EDX,
            argument,
        }
    }

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

    #[test]
    fn character_predicates_cover_all_bytes_unicode_units_and_full_word_boundaries() {
        // The core's Thai word grouping uses U+0E00..U+0E7F. The native predicate compares a full word.
        for value in (0..=0xFFFFu32).chain([0x0100_0E00, 0x8000_0E00, u32::MAX]) {
            let got = invoke(
                fc4p_009100de as *const () as usize,
                0x1234_5678,
                &[value],
                ENTRY_EAX,
                ENTRY_EDX,
                true,
            );
            assert_eq!(
                got,
                [u32::from((0xE00..=0xE7F).contains(&value)), 0x1234_5678, ENTRY_EDX]
            );
        }
        for high in [0, 0x1234_5600, 0xFFFF_FF00] {
            for byte in 0..=255u32 {
                let got = invoke(
                    fc4p_009103b0 as *const () as usize,
                    0x1234_5678,
                    &[high | byte],
                    ENTRY_EAX,
                    ENTRY_EDX,
                    true,
                );
                assert_eq!(got, [u32::from((0xC0..=0xFD).contains(&byte)), 0x1234_5678, ENTRY_EDX]);
            }
        }
    }

    #[test]
    fn reference_increments_wrap_and_keep_every_other_byte() {
        for (f, offset, conditional) in [
            (fc4p_00945942 as *const () as usize, 0x14, false),
            (fc4p_0094bc3f as *const () as usize, 0x20, false),
            (fc4p_0094c4b1 as *const () as usize, 8, false),
            (fc4p_0094d34f as *const () as usize, 0x10, false),
            (fc4p_009334b1 as *const () as usize, 0xC, true),
        ] {
            for flag in [0u8, 1, 0xFF] {
                for count in [0u32, 1, 2, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
                    let mut object = [0xA5; 0x80];
                    object[9] = flag;
                    object[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
                    let mut expected = object;
                    let result = if conditional && flag == 0 {
                        1
                    } else {
                        let next = count.wrapping_add(1);
                        expected[offset..offset + 4].copy_from_slice(&next.to_le_bytes());
                        next
                    };
                    let this = object.as_mut_ptr() as u32;
                    assert_eq!(run(f, this, &[]), [result, this, ENTRY_EDX]);
                    assert_eq!(object, expected);
                }
            }
        }
    }

    #[test]
    fn releases_use_original_signedness_interface_and_destructor_slot() {
        for (f, offset, signed, adjustment, slot, conditional) in [
            (fc4p_00920bf0 as *const () as usize, 0x20, false, 8, 0xC, false),
            (fc4p_00943917 as *const () as usize, 4, true, 0, 0x44, false),
            (fc4p_009745b0 as *const () as usize, 4, false, 0, 0x10, false),
            (fc4p_009334c2 as *const () as usize, 0xC, false, 0, 0x20, true),
        ] {
            for flag in [0u8, 1, 0xFF] {
                for count in [0u32, 1, 2, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
                    reset();
                    let mut vt = [0usize; 20];
                    vt[slot / 4] = fake_one as *const () as usize;
                    let mut object = [0xA5; 0x80];
                    object[9] = flag;
                    object[adjustment..adjustment + 4].copy_from_slice(&(vt.as_ptr() as u32).to_le_bytes());
                    object[offset..offset + 4].copy_from_slice(&count.to_le_bytes());
                    let mut expected = object;
                    let this = object.as_mut_ptr() as u32;
                    let enabled = !conditional || flag != 0;
                    let decrement = if signed { (count as i32) > 1 } else { count > 1 };
                    let want = if !enabled {
                        [1, this, ENTRY_EDX]
                    } else if decrement {
                        expected[offset..offset + 4].copy_from_slice(&count.wrapping_sub(1).to_le_bytes());
                        [count.wrapping_sub(1), this, ENTRY_EDX]
                    } else {
                        [0, RESULT_ECX, RESULT_EDX]
                    };
                    assert_eq!(run(f, this, &[]), want);
                    assert_eq!(object, expected);
                    let trace = calls();
                    if enabled && !decrement {
                        assert_eq!(
                            trace,
                            [expected_call(this + adjustment as u32, vt.as_ptr() as u32, Some(1))]
                        );
                    } else {
                        assert!(trace.is_empty());
                    }
                }
            }
        }
    }

    #[test]
    fn nullable_interface_offsets_keep_the_original_arithmetic_registers() {
        for this in [0u32, 1, 0x124, 0x1000, 0xFFFF_FFF4, u32::MAX] {
            let adjusted = this.wrapping_add(0xC);
            assert_eq!(
                run(fc4p_00968303 as *const () as usize, this, &[]),
                [if this == 0 { 0 } else { adjusted }, adjusted, ENTRY_EDX]
            );
            let secondary = this.wrapping_sub(0x4C);
            assert_eq!(
                run(fc4p_009c3eb7 as *const () as usize, this, &[]),
                [if this == 0x124 { 0 } else { secondary }, secondary, ENTRY_EDX]
            );
        }
    }

    #[test]
    fn directory_type_lookup_checks_unsigned_bounds_before_access() {
        let mut table: [u32; 80] = core::array::from_fn(|i| 0xFEDC_BA00 | i as u32);
        let before = table;
        {
            let _table = crate::testing::fake_data(0x00B1_69F8, table.as_mut_ptr().cast());
            for index in 0..80u32 {
                assert_eq!(
                    invoke(
                        fc4p_009377c9 as *const () as usize,
                        0x1234,
                        &[index],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true
                    ),
                    [table[index as usize], 0x1234, ENTRY_EDX]
                );
            }
        }
        for index in [80u32, 81, 0x8000_0000, u32::MAX] {
            assert_eq!(
                invoke(
                    fc4p_009377c9 as *const () as usize,
                    0x1234,
                    &[index],
                    ENTRY_EAX,
                    ENTRY_EDX,
                    true
                ),
                [0, 0x1234, ENTRY_EDX]
            );
        }
        assert_eq!(table, before);
    }

    #[test]
    fn record_interface_copy_handles_null_and_overlapping_outputs() {
        assert_eq!(
            run(fc4p_00937ef1 as *const () as usize, 0x1234, &[0]),
            [0, 0x1234, ENTRY_EDX]
        );
        for output_offset in [0usize, 4, 8, 12, 20] {
            let mut object = [0xA5; 40];
            object[4..8].copy_from_slice(&0xF123_4567u32.to_le_bytes());
            let mut expected = object;
            expected[output_offset..output_offset + 4].copy_from_slice(&0x00AD_25FCu32.to_le_bytes());
            let second = u32::from_le_bytes(expected[4..8].try_into().unwrap());
            expected[output_offset + 4..output_offset + 8].copy_from_slice(&second.to_le_bytes());
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(
                    fc4p_00937ef1 as *const () as usize,
                    this,
                    &[this + output_offset as u32]
                ),
                [this + output_offset as u32, second, ENTRY_EDX]
            );
            assert_eq!(object, expected);
        }
    }

    #[test]
    fn key_getters_read_each_word_after_the_previous_output_write() {
        for (f, key_offset) in [
            (fc4p_009753ea as *const () as usize, 0xC),
            (fc4p_00975666 as *const () as usize, 8),
        ] {
            for output_offset in [0usize, 4, 8, 12, 16, 20, 24, 40] {
                let mut object: [u8; 64] = core::array::from_fn(|i| i as u8);
                let mut expected = object;
                let mut loaded = [0u32; 3];
                for (i, value) in loaded.iter_mut().enumerate() {
                    let start = key_offset + i * 4;
                    *value = u32::from_le_bytes(expected[start..start + 4].try_into().unwrap());
                    let dest = output_offset + i * 4;
                    expected[dest..dest + 4].copy_from_slice(&value.to_le_bytes());
                }
                let this = object.as_mut_ptr() as u32;
                assert_eq!(
                    run(f, this, &[this + output_offset as u32]),
                    [this + output_offset as u32, loaded[2], loaded[1]]
                );
                assert_eq!(object, expected);
            }
        }
    }

    #[test]
    fn key_setter_keeps_third_word_upper_bytes_and_second_word_edx() {
        for source_offset in [0usize, 4, 8, 12, 16, 20, 24, 40] {
            let mut object: [u8; 64] = core::array::from_fn(|i| (i + 0x80) as u8);
            let mut expected = object;
            let mut loaded = [0u32; 3];
            for (i, value) in loaded.iter_mut().enumerate() {
                let start = source_offset + i * 4;
                *value = u32::from_le_bytes(expected[start..start + 4].try_into().unwrap());
                let dest = 0xC + i * 4;
                expected[dest..dest + 4].copy_from_slice(&value.to_le_bytes());
            }
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(
                    fc4p_00975402 as *const () as usize,
                    this,
                    &[this + source_offset as u32]
                ),
                [(loaded[2] & !255) | 1, this, loaded[1]]
            );
            assert_eq!(object, expected);
        }
    }

    #[test]
    fn key_equality_short_circuits_and_retains_last_loaded_words() {
        let mut object = [0x1234_5678u32, 0x9ABC_DEF0, 0xFEDC_BA98];
        let this = object.as_mut_ptr() as u32;
        for different in 0..=3 {
            let mut other = object;
            if different < 3 {
                other[different] ^= 0x8000_0000;
            }
            let object_before = object;
            let other_before = other;
            let result = run(fc4p_0096d895 as *const () as usize, this, &[other.as_mut_ptr() as u32]);
            assert_eq!(
                result,
                [
                    u32::from(different == 3),
                    if different < 2 { this } else { object[2] },
                    if different == 0 { object[0] } else { object[1] }
                ]
            );
            assert_eq!(object, object_before);
            assert_eq!(other, other_before);
        }
        assert_eq!(
            run(fc4p_0096d895 as *const () as usize, this, &[this]),
            [1, object[2], object[1]]
        );
    }

    #[test]
    fn retained_pointer_is_stored_before_addref_and_null_is_not_called() {
        for present in [false, true] {
            reset();
            let mut vt = [0usize; 2];
            vt[1] = fake_zero as *const () as usize;
            let mut retained = [vt.as_ptr() as u32, 0];
            let mut holder = [0xAAAA_AAAAu32, 0xBBBB_BBBB];
            let this = holder.as_mut_ptr() as u32;
            let value = if present { retained.as_mut_ptr() as u32 } else { 0 };
            STATE.with(|s| s.borrow_mut().watch = Some((this, 8)));
            let result = run(fc4p_0096e773 as *const () as usize, this, &[value]);
            assert_eq!(holder, [value, 0xBBBB_BBBB]);
            if present {
                assert_eq!(result, [this, RESULT_ECX, RESULT_EDX]);
                assert_eq!(calls(), [expected_call(value, vt.as_ptr() as u32, None)]);
                STATE.with(|s| assert_eq!(&s.borrow().snapshots[0][..4], &value.to_le_bytes()));
            } else {
                assert_eq!(result, [this, 0, ENTRY_EDX]);
                assert!(calls().is_empty());
            }
        }
    }

    #[test]
    fn retain_getter_reloads_the_pointer_after_the_callback() {
        for present in [false, true] {
            reset();
            let mut vt = [0usize; 2];
            vt[1] = fake_zero as *const () as usize;
            let mut retained = [vt.as_ptr() as u32, 0];
            let value = if present { retained.as_mut_ptr() as u32 } else { 0 };
            let mut holder = [0xAAAA_AAAAu32, value];
            let this = holder.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().mutation = Some((this, 4, 0x8765_4321)));
            let result = run(fc4p_009779c9 as *const () as usize, this, &[]);
            if present {
                assert_eq!(result, [0x8765_4321, RESULT_ECX, RESULT_EDX]);
                assert_eq!(calls(), [expected_call(value, vt.as_ptr() as u32, None)]);
                assert_eq!(holder, [0xAAAA_AAAA, 0x8765_4321]);
            } else {
                assert_eq!(result, [0, this, ENTRY_EDX]);
                assert!(calls().is_empty());
                assert_eq!(holder, [0xAAAA_AAAA, 0]);
            }
        }
    }

    #[test]
    fn word_and_mask_predicates_normalize_all_bits_without_writes() {
        for bits in [0u32, 1, 0x100, 0x8000_0000, u32::MAX] {
            let mut object = [0xAAAA_AAAAu32, bits, bits, 0xBBBB_BBBB];
            let before = object;
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_0096e957 as *const () as usize, this, &[]),
                [u32::from(bits != 0), this, ENTRY_EDX]
            );
            for mask in [0u32, 1, 0xFF, 0x100, 0x8000_0000, u32::MAX] {
                assert_eq!(
                    run(fc4p_00999055 as *const () as usize, this, &[mask]),
                    [u32::from(bits & mask != 0), this, ENTRY_EDX]
                );
            }
            assert_eq!(object, before);
        }
    }

    #[test]
    fn window_initialization_sets_the_byte_before_its_single_virtual_call() {
        for flag in [0u8, 1, 0xFF] {
            reset();
            let mut vt = [0usize; 93];
            vt[0x170 / 4] = fake_zero as *const () as usize;
            let mut object = [0xA5; 0x120];
            object[4..8].copy_from_slice(&(vt.as_ptr() as u32).to_le_bytes());
            object[0xEC] = flag;
            let mut expected = object;
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 0x120)));
            let result = run(fc4p_009a5faa as *const () as usize, this, &[]);
            if flag == 0 {
                expected[0xEC] = 1;
                assert_eq!(result, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                assert_eq!(calls(), [expected_call(this + 4, vt.as_ptr() as u32, None)]);
                STATE.with(|s| assert_eq!(s.borrow().snapshots, [expected.to_vec()]));
            } else {
                assert_eq!(result, [this + 0xEC, this, ENTRY_EDX]);
                assert!(calls().is_empty());
            }
            assert_eq!(object, expected);
            assert_eq!(
                run(fc4p_009a5faa as *const () as usize, this, &[]),
                [this + 0xEC, this, ENTRY_EDX]
            );
            assert!(calls().is_empty());
        }
    }
}
