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

layout! {
    /// A three-word key. Accesses remain sequential when source and destination overlap.
    struct ThreeWordKey {
        0x00 => first / set_first: u32,
        0x04 => second / set_second: u32,
        0x08 => third / set_third: u32,
    }
}

layout! {
    /// Reference counting is optional for this object shape.
    struct FlaggedReference {
        0x09 => enabled / set_enabled: u8,
        0x0C => references / set_references: u32,
    }
}

layout! {
    /// Initialized by the stream constructor's direct field stores.
    #[allow(dead_code)]
    struct StreamState {
        0x00 => vtable / set_vtable: u32,
        0x04 => count / set_count: u32,
        0x08 => status / set_status: u8,
    }
}

layout! {
    /// Two independent byte options; their argument words are never normalized.
    #[allow(dead_code)]
    struct ByteOptions {
        0x0E => first / set_first: u8,
        0x0F => second / set_second: u8,
    }
}

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
        let key = ThreeWordKey::from_ptr((r.ecx as *mut u8).wrapping_add(offset));
        r.edx = key.first();
        r.eax = r.argument(0);
        let output = ThreeWordKey::from_ptr(r.eax as *mut u8);
        output.set_first(r.edx);
        r.edx = key.second();
        output.set_second(r.edx);
        r.ecx = key.third();
        output.set_third(r.ecx);
    }
}

game_data! { static directory_type_table: [u32; 80] = 0x00B1_69F8; }

/// Tail entries forward the original argument words and all return registers directly to the game.
macro_rules! thunk_port {
    ($address:literal, $export:ident, $callee:ident, $name:literal, $target:literal,
     ($($arg:ident: $ty:ty),*), $($instruction:literal),+ $(,)?) => {
        game_fn! {
            #[allow(dead_code)]
            fn $callee(this: *mut u8 $(, $arg: $ty)*) -> u64 = "thiscall" $target;
        }
        port! {
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!($($instruction,)+
                    "jmp dword ptr [2f]", "2: .long {target}", target = const $target);
                #[cfg(test)]
                core::arch::naked_asm!($($instruction,)+
                    "push {target}", "jmp {dispatch}",
                    target = const $target, dispatch = sym test_tail_dispatch);
            }
        }
    };
}

#[cfg(test)]
extern "fastcall" fn resolve_tail(address: u32, _edx: u32) -> usize {
    crate::testing::resolve(address)
}

#[cfg(test)]
#[unsafe(naked)]
unsafe extern "C" fn test_tail_dispatch() {
    core::arch::naked_asm!(
        "push edx", "push ecx", "push eax", "mov ecx, [esp + 12]", "call {resolve}",
        "mov [esp + 12], eax", "pop eax", "pop ecx", "pop edx", "ret",
        resolve = sym resolve_tail,
    );
}

thunk_port!(
    0x0090dc2f,
    fc4p_0090dc2f,
    destroy_embedded_text,
    "DestroyTextContainer",
    0x0093BAEF,
    (),
    "mov dword ptr [ecx], 0xace650",
    "add ecx, 4"
);
thunk_port!(
    0x00934742,
    fc4p_00934742,
    add_directory_service_reference,
    "AddDirectoryServiceReference",
    0x0093784B,
    (),
    "sub ecx, 0x1c"
);
thunk_port!(
    0x0093474a,
    fc4p_0093474a,
    release_directory_service_reference,
    "ReleaseDirectoryServiceReference",
    0x00937850,
    (),
    "sub ecx, 0x1c"
);
thunk_port!(
    0x009359bd,
    fc4p_009359bd,
    add_primary_reference,
    "AddPrimaryReference",
    0x0094C47C,
    (),
    "sub ecx, 4"
);
thunk_port!(
    0x0093e552,
    fc4p_0093e552,
    add_stream_reference,
    "AddStreamReference",
    0x0093A897,
    (),
    "sub ecx, 4"
);
thunk_port!(
    0x0093e55a,
    fc4p_0093e55a,
    release_stream_reference,
    "ReleaseStreamReference",
    0x0093A89E,
    (),
    "sub ecx, 4"
);
thunk_port!(
    0x0093e5fa,
    fc4p_0093e5fa,
    release_secondary_stream_reference,
    "ReleaseSecondaryStreamReference",
    0x0093A89E,
    (),
    "sub ecx, 0x1c"
);
thunk_port!(
    0x0095ba81,
    fc4p_0095ba81,
    add_window_reference,
    "AddWindowReference",
    0x008FB729,
    (),
    "sub ecx, 0xd8"
);
thunk_port!(
    0x0095ba8c,
    fc4p_0095ba8c,
    release_window_reference,
    "ReleaseWindowReference",
    0x008FB72E,
    (),
    "sub ecx, 0xd8"
);
thunk_port!(
    0x0097569c,
    fc4p_0097569c,
    destroy_record_member,
    "DestroyRecordMember",
    0x00930B02,
    (),
    "mov dword ptr [ecx], 0xad8ca0",
    "add ecx, 8"
);
thunk_port!(
    0x00975a70,
    fc4p_00975a70,
    release_directory_record_reference,
    "ReleaseDirectoryRecordReference",
    0x009759CE,
    (),
    "add ecx, 4"
);
thunk_port!(0x00991e80, fc4p_00991e80, delete_secondary_reference, "DeleteSecondaryReference", 0x00991E90,
    (flags: u32), "sub ecx, 4");
thunk_port!(0x009c409f, fc4p_009c409f, query_scrollbar_interface, "QueryScrollbarInterface", 0x009C38FD,
    (iid: u32, output: *mut *mut u8), "sub ecx, 0x124");
thunk_port!(
    0x009c40b5,
    fc4p_009c40b5,
    release_scrollbar_window_reference,
    "ReleaseScrollbarWindowReference",
    0x008FB72E,
    (),
    "sub ecx, 0x124"
);
thunk_port!(
    0x009c40c0,
    fc4p_009c40c0,
    get_primary_window_interface,
    "GetPrimaryWindowInterface",
    0x0095BA6F,
    (),
    "sub ecx, 0x4c"
);
thunk_port!(0x009c40c8, fc4p_009c40c8, set_scrollbar_position, "SetScrollbarPosition", 0x009C3EAD,
    (position: i32), "sub ecx, 0x4c");
thunk_port!(
    0x009cc22d,
    fc4p_009cc22d,
    release_web_service_reference,
    "ReleaseWebServiceReference",
    0x0091DD89,
    (),
    "add ecx, 8"
);
thunk_port!(
    0x009d6090,
    fc4p_009d6090,
    add_button_reference,
    "AddButtonReference",
    0x0094C47C,
    (),
    "add ecx, 4"
);
thunk_port!(
    0x009d6098,
    fc4p_009d6098,
    release_button_reference,
    "ReleaseButtonReference",
    0x0099B7A3,
    (),
    "add ecx, 4"
);
thunk_port!(
    0x009d6608,
    fc4p_009d6608,
    add_secondary_button_reference,
    "AddSecondaryButtonReference",
    0x009D6090,
    (),
    "sub ecx, 4"
);
thunk_port!(
    0x009d6610,
    fc4p_009d6610,
    release_secondary_button_reference,
    "ReleaseSecondaryButtonReference",
    0x009D6098,
    (),
    "sub ecx, 4"
);
thunk_port!(0x009dae69, fc4p_009dae69, query_window_manager_interface, "QueryWindowManagerInterface", 0x009DAF24,
    (iid: u32, output: *mut *mut u8), "sub ecx, 4");

register_port!(
    0x0090e230,
    fc4p_0090e230,
    forward_value_and_return_this,
    "ForwardValueAndReturnThis",
    4,
    |r| {
        // SAFETY: the object's slot at 56 takes the original stack word.
        unsafe {
            let object = r.ecx;
            let value = r.argument(0);
            r.eax = word(object, 0);
            r.virtual_call(object, 0x38, &[value]);
            r.eax = object;
        }
    }
);

unsafe fn set_scrollbar_limit(r: &mut Registers, offset: usize) {
    // SAFETY: this is the embedded scrollbar interface; the enclosing window has both notification slots.
    unsafe {
        let window = r.ecx.wrapping_sub(0xD8);
        r.eax = r.argument(0);
        put_word(r.ecx, offset, r.eax);
        r.eax = word(window, 0);
        r.virtual_call(window, 0x264, &[]);
        r.eax = word(window, 0);
        r.virtual_call(window, 0x170, &[]);
    }
}

register_port!(
    0x009c3286,
    fc4p_009c3286,
    set_scrollbar_minimum,
    "SetScrollbarMinimum",
    4,
    |r| {
        // SAFETY: the embedded scrollbar's minimum and enclosing window.
        unsafe { set_scrollbar_limit(r, 8) };
    }
);
register_port!(
    0x009c32b0,
    fc4p_009c32b0,
    set_scrollbar_maximum,
    "SetScrollbarMaximum",
    4,
    |r| {
        // SAFETY: the embedded scrollbar's maximum and enclosing window.
        unsafe { set_scrollbar_limit(r, 0xC) };
    }
);

register_port!(
    0x009c3f8f,
    fc4p_009c3f8f,
    set_two_window_values,
    "SetTwoWindowValues",
    8,
    |r| {
        // SAFETY: the enclosing interface has one-word methods at 24 and 32; its table is reloaded between calls.
        unsafe {
            let window = r.ecx.wrapping_sub(0x4C);
            let first = r.argument(0);
            r.eax = word(window, 0);
            r.virtual_call(window, 0x18, &[first]);
            let second = r.argument(1);
            r.eax = word(window, 0);
            r.virtual_call(window, 0x20, &[second]);
            r.eax = (r.eax & !255) | 1;
        }
    }
);

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
            if FlaggedReference::from_ptr(r.ecx as *mut u8).enabled() != 0 {
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
            if FlaggedReference::from_ptr(r.ecx as *mut u8).enabled() != 0 {
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
        let other = ThreeWordKey::from_ptr(r.argument(0) as *mut u8);
        let key = ThreeWordKey::from_ptr(r.ecx as *mut u8);
        r.edx = key.first();
        r.eax = 0;
        if r.edx == other.first() {
            r.edx = key.second();
            if r.edx == other.second() {
                r.ecx = key.third();
                r.eax = u32::from(r.ecx == other.third());
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
        let source = ThreeWordKey::from_ptr(r.argument(0) as *mut u8);
        let key = ThreeWordKey::from_ptr((r.ecx as *mut u8).add(0xC));
        r.edx = source.first();
        key.set_first(r.edx);
        r.edx = source.second();
        key.set_second(r.edx);
        r.eax = source.third();
        key.set_third(r.eax);
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

// These helpers retain the original volatile registers, including partial-byte results.
register_port!(
    0x0090b2f6,
    fc4p_0090b2f6,
    send_default_message,
    "SendDefaultMessage",
    12,
    |r| {
        // SAFETY: the matching message uses this object's five-word dispatch slot.
        unsafe {
            r.eax = 0x856D_DBAC;
            if r.argument(0) == r.eax {
                r.edx = word(r.ecx, 0x14);
                let args = [r.eax, r.argument(1), r.argument(2), r.edx, 0];
                r.virtual_call(r.ecx, 0x10, &args);
            } else {
                r.eax &= !255;
            }
        }
    }
);

register_port!(
    0x0090cff9,
    fc4p_0090cff9,
    move_node_to_front,
    "MoveNodeToFront",
    4,
    |r| {
        // SAFETY: the node and list links are accessible, including when the node is already first.
        unsafe {
            let list = r.ecx;
            r.eax = r.argument(0);
            put_word(r.eax, 0xC, word(r.eax, 0xC).wrapping_add(0xC000_0000));
            r.edx = word(r.eax, 4);
            let next = word(r.eax, 0);
            put_word(r.edx, 0, next);
            put_word(next, 4, r.edx);
            if word(list, 8) == r.eax {
                if r.edx == r.eax {
                    r.edx = 0;
                }
                put_word(list, 8, r.edx);
            }
            r.ecx = word(list, 4);
            r.edx = word(r.ecx, 4);
            put_word(r.eax, 0, r.ecx);
            put_word(r.eax, 4, r.edx);
            put_word(r.ecx, 4, r.eax);
            put_word(r.edx, 0, r.eax);
        }
    }
);

register_port!(
    0x0090d031,
    fc4p_0090d031,
    unlink_and_release_node,
    "UnlinkAndReleaseNode",
    4,
    |r| {
        // SAFETY: the circular links and the list's release slot belong to the supplied live objects.
        unsafe {
            r.edx = r.argument(0);
            r.eax = word(r.edx, 4);
            if r.edx != r.eax {
                let next = word(r.edx, 0);
                put_word(next, 4, r.eax);
                put_word(r.eax, 0, next);
                if word(r.ecx, 4) == r.edx {
                    put_word(r.ecx, 4, r.eax);
                }
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 0xC, &[r.edx]);
            }
        }
    }
);

register_port!(
    0x009133ab,
    fc4p_009133ab,
    release_three_services,
    "ReleaseThreeServices",
    0,
    |r| {
        // SAFETY: each nonnull field owns an interface with a no-argument release slot.
        unsafe {
            let object = r.ecx;
            put_word(object, 0, 0x00AC_EC00);
            for offset in [0x28, 0x24, 0x20] {
                r.ecx = word(object, offset);
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 8, &[]);
                }
            }
        }
    }
);

register_port!(
    0x00940627,
    fc4p_00940627,
    copy_retained_interface,
    "CopyRetainedInterface",
    4,
    |r| {
        // SAFETY: the destination and source expose a table word and one interface field.
        unsafe {
            let object = r.ecx;
            r.eax = r.argument(0);
            put_word(object, 0, 0x00AD_43D8);
            r.ecx = word(r.eax, 4);
            put_word(object, 4, r.ecx);
            if r.ecx != 0 {
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 4, &[]);
            }
            r.eax = object;
        }
    }
);

register_port!(
    0x00940db5,
    fc4p_00940db5,
    assign_matching_type,
    "AssignMatchingType",
    4,
    |r| {
        // SAFETY: both nonnull objects provide a type getter and a four-byte value at +4.
        unsafe {
            let object = r.ecx;
            let source = r.argument(0);
            let mut matches = false;
            if source != 0 {
                r.eax = word(source, 0);
                r.virtual_call(source, 0x10, &[]);
                let source_type = r.eax;
                r.eax = word(object, 0);
                r.virtual_call(object, 0x10, &[]);
                matches = r.eax == source_type;
            }
            if matches {
                r.eax = word(source, 4);
                put_word(object, 4, r.eax);
                r.eax = (r.eax & !255) | 1;
            } else {
                r.eax &= !255;
            }
        }
    }
);

register_port!(
    0x009446e4,
    fc4p_009446e4,
    release_owned_buffer,
    "ReleaseOwnedBuffer",
    0,
    |r| {
        // SAFETY: the buffer's owner provides deallocation and release methods.
        unsafe {
            let object = r.ecx;
            r.ecx = word(object, 0x10);
            let buffer = word(object, 0);
            r.eax = word(r.ecx, 0);
            r.virtual_call(r.ecx, 0x10, &[buffer]);
            r.ecx = word(object, 0x10);
            for offset in [8, 4, 0] {
                put_word(object, offset, 0);
            }
            r.eax = word(r.ecx, 0);
            r.virtual_call(r.ecx, 8, &[]);
            put_word(object, 0x10, 0);
        }
    }
);

register_port!(
    0x0094909a,
    fc4p_0094909a,
    replace_retained_interface,
    "ReplaceRetainedInterface",
    4,
    |r| {
        // SAFETY: each nonnull pointer has the original AddRef and Release slots.
        unsafe {
            let object = r.ecx;
            let old = word(object, 0);
            let new = r.argument(0);
            if new != old {
                if new != 0 {
                    r.eax = word(new, 0);
                    r.virtual_call(new, 4, &[]);
                }
                put_word(object, 0, new);
                if old != 0 {
                    r.eax = word(old, 0);
                    r.virtual_call(old, 8, &[]);
                }
            }
            r.eax = object;
        }
    }
);

register_port!(
    0x0096e78b,
    fc4p_0096e78b,
    copy_five_word_record,
    "CopyFiveWordRecord",
    4,
    |r| {
        // SAFETY: source and destination cover the original sequential word accesses.
        unsafe {
            r.eax = r.ecx;
            r.ecx = r.argument(0);
            let flags: u32;
            core::arch::asm!("pushfd", "pop {flags}", flags = out(reg) flags, options(preserves_flags));
            let stride = if flags & 0x400 == 0 { 4u32 } else { 0u32.wrapping_sub(4) };
            for i in 0u32..3 {
                let offset = i.wrapping_mul(stride) as usize;
                put_word(r.eax, offset, word(r.ecx, offset));
            }
            r.edx = word(r.ecx, 0xC);
            put_word(r.eax, 0xC, r.edx);
            r.ecx = word(r.ecx, 0x10);
            put_word(r.eax, 0x10, r.ecx);
        }
    }
);

register_port!(0x0096edf7, fc4p_0096edf7, swap_three_words, "SwapThreeWords", 4, |r| {
    // SAFETY: both three-word regions are accessible; overlapping accesses stay ordered.
    unsafe {
        r.eax = r.argument(0);
        for offset in [0, 4, 8] {
            if offset == 0 {
                r.edx = word(r.ecx, offset);
            }
            let source = word(r.eax, offset);
            if offset != 0 {
                r.edx = word(r.ecx, offset);
            }
            put_word(r.ecx, offset, source);
            put_word(r.eax, offset, r.edx);
        }
    }
});

register_port!(
    0x0098129c,
    fc4p_0098129c,
    find_unsigned_tree_key,
    "FindUnsignedTreeKey",
    4,
    |r| {
        // SAFETY: the tree header and nonnull links have parent, child and key words at +4/+8/+C/+10.
        unsafe {
            r.edx = word(r.ecx, 0);
            r.ecx = word(r.edx, 4);
            let key = r.argument(0);
            r.eax = r.edx;
            if r.ecx != 0 {
                let wanted = word(key, 0);
                loop {
                    if word(r.ecx, 0x10) >= wanted {
                        r.eax = r.ecx;
                        r.ecx = word(r.ecx, 8);
                    } else {
                        r.ecx = word(r.ecx, 0xC);
                    }
                    if r.ecx == 0 {
                        break;
                    }
                }
            }
            if r.eax != r.edx {
                r.ecx = word(key, 0);
                if r.ecx < word(r.eax, 0x10) {
                    r.eax = r.edx;
                }
            }
        }
    }
);

register_port!(
    0x00a6d999,
    fc4p_00a6d999,
    accumulate_big_endian_bytes,
    "AccumulateBigEndianBytes",
    0,
    |r| {
        // SAFETY: the input contains the requested number of bytes. Longer inputs wrap to the last four bytes.
        unsafe {
            r.edx = r.argument(1);
            r.ecx = r.argument(0);
            r.eax = 0;
            while r.edx != 0 {
                let byte = (r.ecx as *const u8).read();
                r.eax = r.eax.wrapping_shl(8).wrapping_add(u32::from(byte));
                r.ecx = r.ecx.wrapping_add(1);
                r.edx = r.edx.wrapping_sub(1);
            }
        }
    }
);

impl Registers {
    /// Call a declared game entry while retaining all three volatile return registers.
    unsafe fn direct_call(&mut self, address: u32, args: &[u32], caller_pops: bool) {
        #[repr(C)]
        struct Call {
            target: usize,
            eax: u32,
            ecx: u32,
            edx: u32,
            args: *const u32,
            count: u32,
            caller_pops: u32,
        }
        let mut call = Call {
            target: crate::abi::target(address),
            eax: self.eax,
            ecx: self.ecx,
            edx: self.edx,
            args: args.as_ptr(),
            count: args.len() as u32,
            caller_pops: u32::from(caller_pops),
        };
        // SAFETY: the enclosing port declares the entry and supplies its exact convention and words.
        unsafe {
            core::arch::asm!(
                "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
                "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
                "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
                "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
                "cmp dword ptr [edi + 24], 0", "je 4f", "mov ecx, [edi + 20]", "lea esp, [esp + ecx*4]", "4:",
                in("edi") &mut call, clobber_abi("C"),
            );
        }
        self.eax = call.eax;
        self.ecx = call.ecx;
        self.edx = call.edx;
    }
}

game_fn! { #[allow(dead_code)] fn destroy_hanging_list(this: *mut u8) -> u32 = "thiscall" 0x0090_86B7; }
game_fn! { #[allow(dead_code)] fn destroy_text_container(this: *mut u8) -> u32 = "thiscall" 0x0090_DC2F; }
game_fn! { #[allow(dead_code)] fn free_scalar(pointer: *mut u8) -> u32 = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn destroy_value_holder(this: *mut u8) -> u32 = "thiscall" 0x0094_002A; }
game_fn! { #[allow(dead_code)] fn destroy_adjusted_object(this: *mut u8) -> u32 = "thiscall" 0x0090_D964; }
game_fn! { #[allow(dead_code)] fn destroy_text_field(this: *mut u8) -> u32 = "thiscall" 0x0041_4250; }
game_fn! { #[allow(dead_code)] fn allocate_array(bytes: u32) -> u32 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free_array(pointer: *mut u8) -> u32 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn initialize_bounds_base(this: *mut u8) -> u32 = "thiscall" 0x009A_B992; }
game_fn! { #[allow(dead_code)] fn initialize_node_member(this: *mut u8, source: u32) -> u32 = "thiscall" 0x0097_BF85; }
game_fn! { #[allow(dead_code)] fn set_fill_word(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0099_BF1F; }
game_fn! { #[allow(dead_code)] fn erase_tree_branch(this: *mut u8, node: u32) -> u32 = "thiscall" 0x009A_84A0; }
game_fn! { #[allow(dead_code)] fn clear_tree(this: *mut u8) -> u32 = "thiscall" 0x009A_8947; }
game_fn! { #[allow(dead_code)] fn destroy_list_member(this: *mut u8) -> u32 = "thiscall" 0x0096_EDBC; }
game_fn! { #[allow(dead_code)] fn destroy_string_member(this: *mut u8) -> u32 = "thiscall" 0x0056_50A0; }
game_fn! { #[allow(dead_code)] fn destroy_vector_range(begin: u32, end: u32) -> u32 = "C" 0x0098_1543; }
game_fn! { #[allow(dead_code)] fn next_hash_bucket(this: *mut u8) -> u32 = "thiscall" 0x0097_C1EA; }
game_fn! { #[allow(dead_code)] fn lookup_hash_node(this: *mut u8, key: u32) -> u32 = "thiscall" 0x0097_C049; }
game_fn! { #[allow(dead_code)] fn reserve_hash_entry(this: *mut u8, count: u32) -> u32 = "thiscall" 0x0096_F614; }
game_fn! { #[allow(dead_code)] fn insert_hash_entry(this: *mut u8, output: u32, value: u32) -> u32 = "thiscall" 0x0096_F209; }
game_fn! { #[allow(dead_code)] fn grow_pointer_storage(this: *mut u8, count: u32, zero: u32) -> u32 = "thiscall" 0x0097_3C0E; }
game_data! { static string_holder_count: u32 = 0x00B6_33B8; }

/// Destroy the original subobject, optionally release its allocation, and return the primary pointer.
unsafe fn deleting_destructor(r: &mut Registers, target: u32, adjustment: u32, null_adjustment: bool, pop_ecx: bool) {
    // SAFETY: each entry supplies its destructor and the same scalar deletion flag word.
    unsafe {
        let object = r.ecx;
        let flags = r.argument(0);
        r.ecx = if null_adjustment && object == 0 {
            0
        } else {
            object.wrapping_add(adjustment)
        };
        r.direct_call(target, &[], false);
        if flags & 1 != 0 {
            r.direct_call(0x005E_5620, &[object], true);
            if pop_ecx {
                r.ecx = object;
            }
        }
        r.eax = object;
    }
}

register_port!(
    0x00908902,
    fc4p_00908902,
    delete_hanging_indent,
    "DeleteHangingIndent",
    4,
    |r| {
        // SAFETY: this entry owns the hanging-indent destructor and its scalar allocation.
        unsafe {
            deleting_destructor(r, 0x0090_86B7, 0, false, true);
        }
    }
);
register_port!(
    0x0090ddd5,
    fc4p_0090ddd5,
    delete_text_container,
    "DeleteTextContainer",
    4,
    |r| {
        // SAFETY: the text container's destructor is declared by its tail adapter above.
        unsafe {
            deleting_destructor(r, 0x0090_DC2F, 0, false, true);
        }
    }
);
register_port!(
    0x0094065e,
    fc4p_0094065e,
    delete_value_holder,
    "DeleteValueHolder",
    4,
    |r| {
        // SAFETY: the value holder and allocation belong to this deleting destructor.
        unsafe {
            deleting_destructor(r, 0x0094_002A, 0, false, true);
        }
    }
);
register_port!(
    0x00940bc5,
    fc4p_00940bc5,
    delete_interface_base,
    "DeleteInterfaceBase",
    4,
    |r| {
        // SAFETY: the table word belongs to this interface base, even when no deletion is requested.
        unsafe {
            let object = r.ecx;
            put_word(object, 0, 0x00AD_251C);
            if r.argument(0) & 1 != 0 {
                r.direct_call(0x005E_5620, &[object], true);
                r.ecx = object;
            }
            r.eax = object;
        }
    }
);
register_port!(
    0x00991e90,
    fc4p_00991e90,
    delete_offset_interface,
    "DeleteOffsetInterface",
    4,
    |r| {
        // SAFETY: the primary pointer has a nullable subobject at +4.
        unsafe {
            deleting_destructor(r, 0x0090_D964, 4, true, false);
        }
    }
);
register_port!(
    0x009be9af,
    fc4p_009be9af,
    delete_text_owner,
    "DeleteTextOwner",
    4,
    |r| {
        // SAFETY: this owner destroys its text member at +0x14 before scalar deletion.
        unsafe {
            deleting_destructor(r, 0x0041_4250, 0x14, false, true);
        }
    }
);

unsafe fn allocate_elements(r: &mut Registers, element_size: u32) {
    // SAFETY: the game's allocator takes a wrapping byte count and the caller checks for zero elements.
    unsafe {
        r.eax = r.argument(0);
        if r.eax != 0 {
            r.eax = r.eax.wrapping_mul(element_size);
            let bytes = r.eax;
            r.direct_call(0x0090_CF54, &[bytes], true);
            r.ecx = bytes;
        } else {
            r.eax = 0;
        }
    }
}
register_port!(
    0x009b3537,
    fc4p_009b3537,
    allocate_forty_byte_elements,
    "AllocateFortyByteElements",
    8,
    |r| {
        // SAFETY: the allocator receives exactly the original count times forty.
        unsafe {
            allocate_elements(r, 40);
        }
    }
);
register_port!(
    0x009db998,
    fc4p_009db998,
    allocate_word_elements,
    "AllocateWordElements",
    4,
    |r| {
        // SAFETY: the allocator receives exactly the original count times four.
        unsafe {
            allocate_elements(r, 4);
        }
    }
);
register_port!(
    0x009abb63,
    fc4p_009abb63,
    initialize_empty_bounds,
    "InitializeEmptyBounds",
    0,
    |r| {
        // SAFETY: the object provides its base constructor and the original bounds fields.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x009A_B992, &[], false);
            put_word(object, 0xC, 0);
            r.eax = 0x7FFF_FFFF;
            r.ecx = 0x8000_0001;
            put_word(object, 0x14, r.eax);
            put_word(object, 0x1C, r.eax);
            put_word(object, 0x10, r.ecx);
            put_word(object, 0x18, r.ecx);
            (object as *mut u8).add(0x20).write(0);
            r.eax = object;
        }
    }
);
register_port!(
    0x009db1a1,
    fc4p_009db1a1,
    initialize_tagged_member,
    "InitializeTaggedMember",
    8,
    |r| {
        // SAFETY: the first word and member constructor belong to this live object.
        unsafe {
            let object = r.ecx;
            r.eax = r.argument(0);
            put_word(object, 0, r.eax);
            r.ecx = object.wrapping_add(4);
            r.direct_call(0x0097_BF85, &[r.argument(1)], false);
            r.eax = object;
        }
    }
);
register_port!(
    0x009bc8b8,
    fc4p_009bc8b8,
    set_fill_and_refresh,
    "SetFillAndRefresh",
    4,
    |r| {
        // SAFETY: the window exposes its fill setter, flag byte and refresh slot.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x0099_BF1F, &[r.argument(0)], false);
            if (object as *const u8).add(0x128).read() & 0x20 != 0 {
                r.eax = word(object, 0);
                r.virtual_call(object, 0x170, &[]);
            }
        }
    }
);
register_port!(0x009a84a0, fc4p_009a84a0, erase_tree_nodes, "EraseTreeNodes", 4, |r| {
    // SAFETY: each branch node has children at +8/+C and is released through the game allocator.
    unsafe {
        let object = r.ecx;
        let mut node = r.argument(0);
        while node != 0 {
            r.ecx = object;
            r.direct_call(0x009A_84A0, &[word(node, 0xC)], false);
            let left = word(node, 8);
            r.direct_call(0x0090_CF63, &[node], true);
            r.ecx = node;
            node = left;
        }
    }
});
register_port!(
    0x009a8947,
    fc4p_009a8947,
    clear_unsigned_tree,
    "ClearUnsignedTree",
    0,
    |r| {
        // SAFETY: the header and count belong to the tree; callbacks can replace the header pointer.
        unsafe {
            let object = r.ecx;
            if word(object, 4) != 0 {
                r.eax = word(object, 0);
                r.direct_call(0x009A_84A0, &[word(r.eax, 4)], false);
                r.eax = word(object, 0);
                put_word(r.eax, 8, r.eax);
                r.eax = word(object, 0);
                put_word(r.eax, 4, 0);
                r.eax = word(object, 0);
                put_word(r.eax, 0xC, r.eax);
                put_word(object, 4, 0);
            }
        }
    }
);
register_port!(
    0x009a9427,
    fc4p_009a9427,
    destroy_unsigned_tree,
    "DestroyUnsignedTree",
    0,
    |r| {
        // SAFETY: clear the tree before releasing its nonnull header allocation.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x009A_8947, &[], false);
            let header = word(object, 0);
            if header != 0 {
                r.direct_call(0x0090_CF63, &[header], true);
                r.ecx = header;
            }
        }
    }
);
register_port!(
    0x009a841f,
    fc4p_009a841f,
    clear_circular_list,
    "ClearCircularList",
    0,
    |r| {
        // SAFETY: the list ends at its header; links are read before each allocation is released.
        unsafe {
            let object = r.ecx;
            r.eax = word(object, 0);
            let mut node = word(r.eax, 0);
            if node != r.eax {
                loop {
                    r.eax = node;
                    node = word(node, 0);
                    let released = r.eax;
                    r.direct_call(0x0090_CF63, &[released], true);
                    let finished = node == word(object, 0);
                    r.ecx = released;
                    if finished {
                        break;
                    }
                }
            }
            r.eax = word(object, 0);
            put_word(r.eax, 0, r.eax);
            let header = word(object, 0);
            put_word(header, 4, header);
        }
    }
);
register_port!(
    0x00945f24,
    fc4p_00945f24,
    destroy_two_members,
    "DestroyTwoMembers",
    0,
    |r| {
        // SAFETY: the object's list and text members have these original destructor entries.
        unsafe {
            let object = r.ecx;
            r.ecx = object.wrapping_add(0x64);
            r.direct_call(0x0096_EDBC, &[], false);
            r.ecx = object.wrapping_add(0xC);
            r.direct_call(0x0041_4250, &[], false);
        }
    }
);
register_port!(
    0x00943a0e,
    fc4p_00943a0e,
    destroy_counted_strings,
    "DestroyCountedStrings",
    0,
    |r| {
        // SAFETY: restore the table and decrease the game counter before destroying the two strings.
        unsafe {
            let object = r.ecx;
            put_word(object, 0, 0x00AD_4648);
            let count = string_holder_count();
            count.write_unaligned(count.read_unaligned().wrapping_sub(1));
            r.ecx = object.wrapping_add(0xC);
            r.direct_call(0x0056_50A0, &[], false);
            r.ecx = object.wrapping_add(8);
            r.direct_call(0x0056_50A0, &[], false);
        }
    }
);
register_port!(
    0x0093de49,
    fc4p_0093de49,
    destroy_word_vector,
    "DestroyWordVector",
    0,
    |r| {
        // SAFETY: the two range words and storage pointer belong to the vector.
        unsafe {
            let object = r.ecx;
            let end = word(object, 4);
            let begin = word(object, 0);
            r.direct_call(0x0098_1543, &[begin, end], true);
            let storage = word(object, 0);
            r.ecx = end;
            if storage != 0 {
                r.direct_call(0x0090_CF63, &[storage], true);
                r.ecx = storage;
            }
        }
    }
);
register_port!(
    0x0096f5b7,
    fc4p_0096f5b7,
    advance_hash_iterator,
    "AdvanceHashIterator",
    0,
    |r| {
        // SAFETY: the iterator points at a node whose first word is the next link.
        unsafe {
            let object = r.ecx;
            r.eax = word(word(object, 0), 0);
            if r.eax == 0 {
                r.direct_call(0x0097_C1EA, &[], false);
            }
            put_word(object, 0, r.eax);
            r.eax = object;
        }
    }
);
register_port!(
    0x0096f290,
    fc4p_0096f290,
    find_hash_iterator,
    "FindHashIterator",
    8,
    |r| {
        // SAFETY: the lookup result and container are written in order to the caller's two-word output.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x0097_C049, &[r.argument(1)], false);
            r.ecx = r.argument(0);
            put_word(r.ecx, 0, r.eax);
            put_word(r.ecx, 4, object);
            r.eax = r.ecx;
        }
    }
);
register_port!(
    0x0097005c,
    fc4p_0097005c,
    reserve_and_insert_hash_entry,
    "ReserveAndInsertHashEntry",
    8,
    |r| {
        // SAFETY: the container's reserve and insertion entries receive the original count and arguments.
        unsafe {
            let object = r.ecx;
            r.eax = word(object, 0x10).wrapping_add(1);
            r.direct_call(0x0096_F614, &[r.eax], false);
            r.ecx = object;
            r.direct_call(0x0096_F209, &[r.argument(0), r.argument(1)], false);
            r.eax = r.argument(0);
        }
    }
);
register_port!(
    0x00982d95,
    fc4p_00982d95,
    ensure_pointer_capacity,
    "EnsurePointerCapacity",
    4,
    |r| {
        // SAFETY: the container's geometry and grow entry use wrapping unsigned capacity comparisons.
        unsafe {
            r.eax = ((word(r.ecx, 0x1C).wrapping_sub(word(r.ecx, 0x20)) as i32) >> 2) as u32;
            r.edx = word(r.ecx, 0x24).wrapping_sub(r.eax);
            r.eax = r.argument(0);
            if r.eax.wrapping_add(1) > r.edx {
                r.direct_call(0x0097_3C0E, &[r.eax, 0], false);
            }
        }
    }
);
register_port!(
    0x009756f6,
    fc4p_009756f6,
    release_locked_reference,
    "ReleaseLockedReference",
    0,
    |r| {
        // SAFETY: the embedded lock and primary cleanup slot are the original object's interfaces.
        unsafe {
            let object = r.ecx;
            let lock = object.wrapping_add(8);
            r.eax = word(lock, 0);
            r.virtual_call(lock, 4, &[]);
            r.eax = word(object, 4);
            r.ecx = lock;
            let remaining = if r.eax > 1 {
                r.eax = r.eax.wrapping_sub(1);
                put_word(object, 4, r.eax);
                r.eax
            } else {
                0
            };
            r.eax = word(lock, 0);
            r.virtual_call(lock, 8, &[]);
            if remaining == 0 {
                r.eax = word(object, 0);
                r.virtual_call(object, 0x7C, &[]);
            }
            r.eax = remaining;
        }
    }
);
register_port!(
    0x0096df8d,
    fc4p_0096df8d,
    stop_and_free_work_buffer,
    "StopAndFreeWorkBuffer",
    0,
    |r| {
        // SAFETY: the state bytes, embedded stream and buffer pointer belong to the active object.
        unsafe {
            let object = r.ecx;
            if (object as *const u8).add(0x14).read() != 0 {
                let needs_stop = (object as *const u8).add(0x1C).read() & 3 != 0;
                (object as *mut u8).add(0x14).write(0);
                if needs_stop {
                    r.ecx = object.wrapping_add(4);
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 0x20, &[]);
                }
                let buffer = word(object, 0x1C4);
                r.direct_call(0x005E_5620, &[buffer], true);
                put_word(object, 0x1C4, 0);
                r.ecx = buffer;
            }
            r.eax = (r.eax & !255) | 1;
        }
    }
);

game_fn! { #[allow(dead_code)] fn current_encoding() -> u32 = "C" 0x0091_0014; }
game_fn! { #[allow(dead_code)] fn is_utf8_lead_byte(byte: u32) -> u32 = "C" 0x0091_03B0; }
game_fn! { #[allow(dead_code)] fn utf8_tail_count(byte: u32) -> u32 = "C" 0x0091_0128; }
game_fn! { #[allow(dead_code)] fn is_codepage_lead_byte(byte: u32, encoding: u32) -> u32 = "C" 0x0091_030F; }
game_fn! { #[allow(dead_code)] fn codepage_tail_count(byte: u32, encoding: u32) -> u32 = "C" 0x0091_036D; }

register_port!(
    0x00910128,
    fc4p_00910128,
    count_utf8_trailing_bytes,
    "CountUtf8TrailingBytes",
    0,
    |r| {
        // SAFETY: only the low argument byte is inspected, including legacy five- and six-byte sequences.
        unsafe {
            r.eax = match r.argument(0) as u8 {
                0xC2..=0xDF => 1,
                0xE0..=0xEF => 2,
                0xF0..=0xF7 => 3,
                0xF8..=0xFB => 4,
                0xFC..=0xFD => 5,
                _ => 0,
            };
        }
    }
);
register_port!(
    0x0091030f,
    fc4p_0091030f,
    check_codepage_lead_byte,
    "CheckCodepageLeadByte",
    0,
    |r| {
        // SAFETY: recognized code pages return full Booleans; others retain the subtraction's upper bytes.
        unsafe {
            let byte = r.argument(0) as u8;
            r.eax = match r.argument(1) {
                932 => u32::from((0x81..=0x9F).contains(&byte) || (0xE0..=0xFC).contains(&byte)),
                936 => u32::from((0xA1..=0xFC).contains(&byte)),
                949 => u32::from((0xB0..=0xC8).contains(&byte)),
                950 => u32::from((0x81..=0xFE).contains(&byte)),
                other => other.wrapping_sub(950) & !255,
            };
        }
    }
);
register_port!(
    0x0091036d,
    fc4p_0091036d,
    count_codepage_trailing_bytes,
    "CountCodepageTrailingBytes",
    0,
    |r| {
        // SAFETY: delegate to the game's predicate and normalize its low byte only.
        unsafe {
            r.direct_call(0x0091_030F, &[r.argument(0), r.argument(1)], true);
            r.eax = u32::from(r.eax as u8 != 0);
        }
    }
);
unsafe fn encoding_character_query(r: &mut Registers, count: bool) {
    // SAFETY: both cdecl queries forward a character word and an encoding word to the original entries.
    unsafe {
        r.eax = r.argument(1);
        let mut utf8 = r.eax == u32::MAX - 1;
        if !utf8 {
            if r.eax == u32::MAX {
                r.direct_call(0x0091_0014, &[], true);
            }
            utf8 = r.eax == 8;
        }
        if utf8 {
            let character = r.argument(0);
            r.direct_call(if count { 0x0091_0128 } else { 0x0091_03B0 }, &[character], true);
            if !count {
                r.ecx = character;
            }
        } else if (r.eax as i32) >= 100 {
            r.direct_call(
                if count { 0x0091_036D } else { 0x0091_030F },
                &[r.argument(0), r.eax],
                true,
            );
        } else if count {
            r.eax = 0;
        } else {
            r.eax &= !255;
        }
    }
}
register_port!(
    0x009108b4,
    fc4p_009108b4,
    query_character_lead_byte,
    "QueryCharacterLeadByte",
    0,
    |r| {
        // SAFETY: retain the low-byte result convention and the UTF-8 branch's ECX cleanup value.
        unsafe {
            encoding_character_query(r, false);
        }
    }
);
register_port!(
    0x009108ed,
    fc4p_009108ed,
    query_character_trailing_count,
    "QueryCharacterTrailingCount",
    0,
    |r| {
        // SAFETY: retain the full count and caller cleanup of both argument words.
        unsafe {
            encoding_character_query(r, true);
        }
    }
);
register_port!(
    0x0097c081,
    fc4p_0097c081,
    begin_hash_iterator,
    "BeginHashIterator",
    4,
    |r| {
        // SAFETY: buckets lie between +4/+8; output's two sequential writes may overlap the container.
        unsafe {
            let object = r.ecx;
            r.eax = ((word(object, 8).wrapping_sub(word(object, 4)) as i32) >> 2) as u32;
            r.edx = 0;
            let mut found = 0;
            if r.eax != 0 {
                r.eax = word(object, 4);
                let mut slot = r.eax;
                loop {
                    if word(slot, 0) != 0 {
                        r.edx = word(r.eax, r.edx.wrapping_mul(4) as usize);
                        found = r.edx;
                        break;
                    }
                    let count = ((word(object, 8).wrapping_sub(word(object, 4)) as i32) >> 2) as u32;
                    r.edx = r.edx.wrapping_add(1);
                    slot = slot.wrapping_add(4);
                    if r.edx >= count {
                        break;
                    }
                }
            }
            r.eax = r.argument(0);
            put_word(r.eax, 0, found);
            put_word(r.eax, 4, object);
        }
    }
);

layout! {
    /// Start, used end and storage limit of a contiguous buffer.
    struct BufferBounds {
        0x00 => begin / set_begin: u32,
        0x04 => end / set_end: u32,
        0x08 => limit / set_limit: u32,
    }
}
layout! {
    /// Resources released when a document becomes inactive; pointer vectors are embedded at +AC and +BC.
    struct DocumentResources {
        0x14 => window / set_window: u32,
        0x18 => active / set_active: u8,
        0x2C => provider / set_provider: u32,
        0x30 => manager / set_manager: u32,
        0xB8 => child_window / set_child_window: u32,
        0x198 => tree_header / set_tree_header: u32,
        0x1A4 => cache / set_cache: u32,
    }
}
layout! {
    /// A resource record can hold either a retained interface or a manager-owned handle.
    struct DocumentResourceNode {
        0x24 => interface / set_interface: u32,
        0x34 => handle / set_handle: u32,
    }
}
layout! {
    /// Owner of the hanging-indent pointer vector, which begins at +1C.
    struct HangingIndentResources {
        0x00 => vtable / set_vtable: u32,
        0x1C => begin / set_begin: u32,
        0x20 => end / set_end: u32,
    }
}
layout! {
    /// A pair of circular rings, one available and one active.
    struct ValueRingPool {
        0x00 => vtable / set_vtable: u32,
        0x04 => available / set_available: u32,
        0x08 => active / set_active: u32,
    }
}
layout! {
    /// Ring links, returned payload and wrapping activation counter.
    struct ValueRingNode {
        0x00 => next / set_next: u32,
        0x04 => previous / set_previous: u32,
        0x08 => value / set_value: u32,
        0x0C => flags / set_flags: u32,
    }
}
layout! {
    /// The text tree starts at +4 and its element count is at +8.
    struct TextTreeContainer {
        0x00 => vtable / set_vtable: u32,
        0x04 => tree_header / set_tree_header: u32,
        0x08 => count / set_count: u32,
    }
}

game_fn! { #[allow(dead_code)] fn insert_list_value(this: *mut u8, output: u32, position: u32, value: u32) -> u32 = "thiscall" 0x0089_7C8D; }
game_fn! { #[allow(dead_code)] fn grow_word_vector(this: *mut u8, position: u32, value: u32, tag: u32, first: u32, count: u32) -> u32 = "thiscall" 0x0099_4D77; }
game_fn! { #[allow(dead_code)] fn clear_pointer_range(this: *mut u8, begin: u32, end: u32) -> u32 = "thiscall" 0x009A_1ADD; }
game_fn! { #[allow(dead_code)] fn advance_resource_node(node: u32) -> u32 = "C" 0x0040_D4C0; }
game_fn! { #[allow(dead_code)] fn clear_resource_tree(this: *mut u8) -> u32 = "thiscall" 0x008F_FD4F; }
game_fn! { #[allow(dead_code)] fn construct_seven_words(destination: u32, source: u32) -> u32 = "C" 0x009A_7610; }
game_fn! { #[allow(dead_code)] fn enter_pool_lock(this: *mut u8) -> u32 = "thiscall" 0x0093_0B35; }
game_fn! { #[allow(dead_code)] fn leave_pool_lock(this: *mut u8) -> u32 = "thiscall" 0x0093_0B6D; }
game_fn! { #[allow(dead_code)] fn allocate_small_block(this: *mut u8, size: u32) -> u32 = "thiscall" 0x0090_D6AB; }
game_fn! { #[allow(dead_code)] fn allocate_large_block(this: *mut u8, size: u32) -> u32 = "thiscall" 0x0090_D494; }
game_fn! { #[allow(dead_code)] fn free_small_block(this: *mut u8, pointer: u32, size: u32) -> u32 = "thiscall" 0x0090_D7F2; }
game_fn! { #[allow(dead_code)] fn free_large_block(this: *mut u8, pointer: u32, size: u32) -> u32 = "thiscall" 0x0090_D5DB; }
game_fn! { #[allow(dead_code)] fn construct_text_tree(this: *mut u8, tag: u32) -> u32 = "thiscall" 0x0093_F7EB; }
game_fn! { #[allow(dead_code)] fn get_retained_value(this: *mut u8, value: u32) -> u32 = "thiscall" 0x004F_20C0; }
game_fn! { #[allow(dead_code)] fn insert_text_tree_value(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0096_90F4; }
game_fn! { #[allow(dead_code)] fn find_error_mapping(begin: u32, end: u32, output: u32, key: u32, flags: u32) -> u32 = "C" 0x0079_FA90; }
game_fn! { #[allow(dead_code)] fn report_bad_string_position() -> u32 = "C" 0x0040_7C40; }
game_fn! { #[allow(dead_code)] fn report_string_length(this: *mut u8) -> u32 = "thiscall" 0x0079_C6E0; }
game_fn! { #[allow(dead_code)] fn insert_string_range(this: *mut u8, position: u32, begin: u32, end: u32) -> u32 = "thiscall" 0x005C_2910; }
game_fn! { #[allow(dead_code)] fn construct_service_guard(this: *mut u8, service: u32) -> u32 = "thiscall" 0x0048_C9F0; }
game_fn! { #[allow(dead_code)] fn release_service_value(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0050_5190; }
game_fn! { #[allow(dead_code)] fn construct_message_key(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0044_A490; }
game_fn! { #[allow(dead_code)] fn allocate_message_object(size: u32) -> u32 = "C" 0x005E_55E0; }
game_fn! { #[allow(dead_code)] fn construct_keyed_message(this: *mut u8, first: u32, second: u32) -> u32 = "thiscall" 0x0097_80DC; }
game_fn! { #[allow(dead_code)] fn next_random_sample(this: *mut u8) -> St0 = "thiscall" 0x0091_372E; }
game_data! { static block_pool_lock: u8 = 0x00B6_25FC; }
game_data! { static small_block_pool: u8 = 0x00B6_25D4; }
game_data! { static large_block_pool: u8 = 0x00B6_25B0; }
game_data! { static error_mapping_begin: u8 = 0x00A8_29B0; }
game_data! { static error_mapping_end: u8 = 0x00A8_2A20; }
game_data! { static value_release_service: u8 = 0x00B6_27E0; }
game_data! { static value_release_guard: u8 = 0x00B6_2870; }
game_data! { static three_sample_scale: f64 = 0x00AC_ECB0; }

impl Registers {
    /// The caller's actual argument word, also used as temporary output by several entries.
    fn argument_pointer(&mut self, index: usize) -> *mut u32 {
        (self as *mut Self).cast::<u32>().wrapping_add(4 + index)
    }
}

register_port!(
    0x008f6e80,
    fc4p_008f6e80,
    insert_at_list_position,
    "InsertAtListPosition",
    4,
    |r| {
        // SAFETY: the object contains the position; the caller's argument is the insertion's temporary output.
        unsafe {
            let value = r.argument(0);
            r.edx = word(r.ecx, 0);
            r.eax = r.argument_pointer(0) as u32;
            r.direct_call(0x0089_7C8D, &[r.eax, r.edx, value], false);
        }
    }
);

register_port!(
    0x008ff785,
    fc4p_008ff785,
    append_word_value,
    "AppendWordValue",
    4,
    |r| {
        // SAFETY: the vector and input word are live; growth receives the original argument's high byte.
        unsafe {
            let vector = BufferBounds::from_ptr(r.ecx as *mut u8);
            r.eax = vector.end();
            let input = r.argument(0);
            if r.eax != vector.limit() {
                if r.eax != 0 {
                    r.edx = word(input, 0);
                    put_word(r.eax, 0, r.edx);
                }
                vector.set_end(vector.end().wrapping_add(4));
            } else {
                r.edx = (r.argument_pointer(0) as u32).wrapping_add(3);
                r.direct_call(0x0099_4D77, &[r.eax, input, r.edx, 1, 1], false);
            }
        }
    }
);

/// Visit each pointer, reloading both bounds after every deleting callback, then clear the range.
unsafe fn delete_pointer_vector(r: &mut Registers, vector: u32) {
    // SAFETY: the owning shutdown supplies a live pointer vector and deleting methods.
    unsafe {
        let bounds = BufferBounds::from_ptr(vector as *mut u8);
        r.eax = ((bounds.end().wrapping_sub(bounds.begin()) as i32) >> 2) as u32;
        if r.eax != 0 {
            let mut index = 0u32;
            loop {
                r.eax = bounds.begin();
                r.ecx = word(r.eax, index.wrapping_mul(4) as usize);
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 0, &[1]);
                }
                r.eax = ((bounds.end().wrapping_sub(bounds.begin()) as i32) >> 2) as u32;
                index = index.wrapping_add(1);
                if index >= r.eax {
                    break;
                }
            }
        }
        r.ecx = vector;
        r.direct_call(0x009A_1ADD, &[bounds.begin(), bounds.end()], false);
    }
}

register_port!(
    0x00900357,
    fc4p_00900357,
    shutdown_document_resources,
    "ShutdownDocumentResources",
    0,
    |r| {
        // SAFETY: resource fields and their interfaces belong to the live document.
        unsafe {
            let object = r.ecx;
            let document = DocumentResources::from_ptr(object as *mut u8);
            if document.active() != 0 {
                r.ecx = document.window();
                document.set_active(0);
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 0, &[1]);
                }
                document.set_window(0);
                r.ecx = document.child_window();
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 0x70, &[1]);
                }
                document.set_child_window(0);
                delete_pointer_vector(r, object.wrapping_add(0xAC));
                delete_pointer_vector(r, object.wrapping_add(0xBC));
                let tree = object.wrapping_add(0x198);
                let header = document.tree_header();
                let mut node = word(header, 8);
                while node != header {
                    let resource = DocumentResourceNode::from_ptr(node as *mut u8);
                    r.ecx = resource.interface();
                    if r.ecx != 0 {
                        r.eax = word(r.ecx, 0);
                        r.virtual_call(r.ecx, 8, &[]);
                    } else {
                        r.ecx = document.manager();
                        r.eax = word(r.ecx, 0);
                        r.virtual_call(r.ecx, 0x58, &[resource.handle()]);
                    }
                    r.direct_call(0x0040_D4C0, &[node], true);
                    r.ecx = node;
                    node = r.eax;
                }
                r.ecx = tree;
                r.direct_call(0x008F_FD4F, &[], false);
                for offset in [0x1A4, 0x30, 0x2C] {
                    r.ecx = word(object, offset);
                    if r.ecx != 0 {
                        r.eax = word(r.ecx, 0);
                        r.virtual_call(r.ecx, 8, &[]);
                        put_word(object, offset, 0);
                    }
                }
            }
            r.eax = (r.eax & !255) | 1;
        }
    }
);

register_port!(
    0x009086b7,
    fc4p_009086b7,
    destroy_hanging_indent_values,
    "DestroyHangingIndentValues",
    0,
    |r| {
        // SAFETY: the captured bounds remain valid while callbacks may change the object's vector fields.
        unsafe {
            let object = r.ecx;
            let owner = HangingIndentResources::from_ptr(object as *mut u8);
            let end = owner.end();
            let vector = object.wrapping_add(0x1C);
            let mut cursor = owner.begin();
            owner.set_vtable(0x00AC_D5E8);
            while cursor != end {
                r.ecx = word(cursor, 0);
                if r.ecx != 0 {
                    r.eax = word(r.ecx, 0);
                    r.virtual_call(r.ecx, 0, &[1]);
                }
                cursor = cursor.wrapping_add(4);
            }
            r.ecx = vector;
            r.direct_call(0x009A_1ADD, &[owner.begin(), owner.end()], false);
            let storage = owner.begin();
            if storage != 0 {
                r.direct_call(0x0090_CF63, &[storage], true);
                r.ecx = storage;
            }
            owner.set_vtable(0x00AC_D504);
        }
    }
);

register_port!(
    0x0090990f,
    fc4p_0090990f,
    copy_seven_word_range,
    "CopySevenWordRange",
    0,
    |r| {
        // SAFETY: each source element and output element spans seven words; the game constructs every element.
        unsafe {
            let mut source = r.argument(0);
            let mut output = r.argument(2);
            while source != r.argument(1) {
                r.direct_call(0x009A_7610, &[output, source], true);
                r.ecx = source;
                source = source.wrapping_add(0x1C);
                output = output.wrapping_add(0x1C);
            }
            r.eax = output;
        }
    }
);

register_port!(
    0x0090b9e7,
    fc4p_0090b9e7,
    create_keyed_message_interface,
    "CreateKeyedMessageInterface",
    20,
    |r| {
        // SAFETY: the supported class uses a two-word key and a 0x38-byte game allocation.
        unsafe {
            if r.argument(0) == 0x856D_DBAC {
                let mut key = [r.ecx; 2];
                r.eax = r.argument(3) & 0xFFFF;
                r.ecx = key.as_mut_ptr() as u32;
                r.direct_call(0x0044_A490, &[r.eax], false);
                r.direct_call(0x005E_55E0, &[0x38], true);
                r.ecx = 0x38;
                if r.eax != 0 {
                    r.ecx = r.eax;
                    r.direct_call(0x0097_80DC, &key, false);
                } else {
                    r.eax = 0;
                }
                r.edx = word(r.eax, 0);
                r.ecx = r.eax;
                r.virtual_call(r.ecx, 0, &[r.argument(1), r.argument(2)]);
            } else {
                r.eax &= !255;
            }
        }
    }
);

register_port!(
    0x0090cf9a,
    fc4p_0090cf9a,
    activate_free_ring_node,
    "ActivateFreeRingNode",
    0,
    |r| {
        // SAFETY: both rings and an optional newly allocated node use the same next/previous layout.
        unsafe {
            let object = r.ecx;
            let pool = ValueRingPool::from_ptr(object as *mut u8);
            let node = pool.available();
            let entry = ValueRingNode::from_ptr(node as *mut u8);
            if node == entry.previous() {
                r.eax = pool.vtable();
                r.virtual_call(object, 8, &[]);
                r.ecx = pool.available();
                r.edx = word(r.ecx, 4);
                put_word(r.edx, 0, r.eax);
                put_word(r.ecx, 4, r.eax);
                put_word(r.eax, 0, r.ecx);
                put_word(r.eax, 4, r.edx);
                pool.set_available(r.eax);
            }
            r.eax = entry.previous();
            r.ecx = entry.next();
            put_word(r.eax, 0, r.ecx);
            put_word(r.ecx, 4, r.eax);
            pool.set_available(r.eax);
            entry.set_flags(entry.flags().wrapping_add(0x4000_0000));
            r.eax = pool.active();
            if r.eax == 0 {
                entry.set_previous(node);
                entry.set_next(node);
            } else {
                r.ecx = word(r.eax, 4);
                entry.set_next(r.eax);
                entry.set_previous(r.ecx);
                put_word(r.ecx, 0, node);
                put_word(r.eax, 4, node);
            }
            r.eax = pool.available();
            pool.set_active(node);
            r.eax = word(r.eax, 8);
        }
    }
);

register_port!(
    0x0090d8e0,
    fc4p_0090d8e0,
    allocate_locked_block,
    "AllocateLockedBlock",
    0,
    |r| {
        // SAFETY: all pool objects and their shared lock are the game's globals.
        unsafe {
            r.ecx = block_pool_lock() as u32;
            r.direct_call(0x0093_0B35, &[], false);
            r.eax = r.argument(0);
            let small = r.eax < 14;
            r.ecx = if small { small_block_pool() } else { large_block_pool() } as u32;
            r.direct_call(if small { 0x0090_D6AB } else { 0x0090_D494 }, &[r.eax], false);
            let result = r.eax;
            r.ecx = block_pool_lock() as u32;
            r.direct_call(0x0093_0B6D, &[], false);
            r.eax = result;
        }
    }
);

register_port!(
    0x0090d91c,
    fc4p_0090d91c,
    free_locked_block,
    "FreeLockedBlock",
    0,
    |r| {
        // SAFETY: select the same pool by the unsigned size, then return the unlock call's registers.
        unsafe {
            r.ecx = block_pool_lock() as u32;
            r.direct_call(0x0093_0B35, &[], false);
            let small = r.argument(1) < 14;
            r.ecx = if small { small_block_pool() } else { large_block_pool() } as u32;
            r.direct_call(
                if small { 0x0090_D7F2 } else { 0x0090_D5DB },
                &[r.argument(0), r.argument(1)],
                false,
            );
            r.ecx = block_pool_lock() as u32;
            r.direct_call(0x0093_0B6D, &[], false);
        }
    }
);

register_port!(
    0x0090dc0f,
    fc4p_0090dc0f,
    construct_text_container,
    "ConstructTextContainer",
    0,
    |r| {
        // SAFETY: construct the member after restoring its owner's table, then clear the count.
        unsafe {
            let object = r.ecx;
            let container = TextTreeContainer::from_ptr(object as *mut u8);
            let mut scratch = r.ecx;
            r.eax = (&mut scratch as *mut u32 as u32).wrapping_add(3);
            r.ecx = object.wrapping_add(4);
            container.set_vtable(0x00AC_E650);
            r.direct_call(0x0093_F7EB, &[r.eax], false);
            container.set_count(0);
            r.eax = object;
            r.ecx = scratch;
        }
    }
);

register_port!(
    0x0090dc3d,
    fc4p_0090dc3d,
    append_retained_text_value,
    "AppendRetainedTextValue",
    4,
    |r| {
        // SAFETY: the retain helper updates the original argument; release uses that updated value after insertion.
        unsafe {
            let object = r.ecx;
            let input = r.argument(0);
            r.ecx = r.argument_pointer(0) as u32;
            r.direct_call(0x004F_20C0, &[input], false);
            r.ecx = object.wrapping_add(4);
            r.direct_call(0x0096_90F4, &[r.eax], false);
            r.ecx = r.argument(0);
            if r.ecx != 0 {
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 8, &[]);
            }
            r.eax = (r.eax & !255) | 1;
        }
    }
);

register_port!(0x0090df7c, fc4p_0090df7c, map_error_code, "MapErrorCode", 4, |r| {
    // SAFETY: the fixed mapping range and the output argument are passed to the game's search helper.
    unsafe {
        r.eax = r.argument_pointer(0) as u32;
        let end = error_mapping_end() as u32;
        r.direct_call(
            0x0079_FA90,
            &[error_mapping_begin() as u32, end, r.eax, r.argument(0), 0],
            true,
        );
        r.eax = if r.eax == end { (-5i32) as u32 } else { word(r.eax, 0) };
    }
});

register_port!(
    0x0090e8b6,
    fc4p_0090e8b6,
    insert_string_bytes,
    "InsertStringBytes",
    12,
    |r| {
        // SAFETY: the string checks may report errors and return; all subsequent fields are then reloaded.
        unsafe {
            let object = r.ecx;
            let bounds = BufferBounds::from_ptr(object as *mut u8);
            r.eax = bounds.end().wrapping_sub(bounds.begin());
            if r.argument(0) > r.eax {
                r.direct_call(0x0040_7C40, &[], true);
            }
            r.eax = bounds.end().wrapping_sub(bounds.begin());
            let count = r.argument(2);
            r.ecx = (u32::MAX - 1).wrapping_sub(count);
            if r.eax > r.ecx {
                r.ecx = object;
                r.direct_call(0x0079_C6E0, &[], false);
            }
            r.eax = r.argument(1);
            r.ecx = r.eax.wrapping_add(count);
            let end = r.ecx;
            let begin = r.eax;
            r.eax = bounds.begin().wrapping_add(r.argument(0));
            r.ecx = object;
            r.direct_call(0x005C_2910, &[r.eax, begin, end], false);
            r.eax = object;
        }
    }
);

register_port!(0x0090ffc8, fc4p_0090ffc8, fill_word_buffer, "FillWordBuffer", 0, |r| {
    // SAFETY: the caller supplies enough writable storage, including the leading unaligned word.
    unsafe {
        let mut output = r.argument(0);
        r.ecx = r.argument(2);
        if r.ecx != 0 {
            r.edx = output & 3;
            r.eax = (r.eax & !0xFFFF) | (r.argument(1) & 0xFFFF);
            if r.edx != 0 {
                (output as *mut u16).write_unaligned(r.eax as u16);
                r.ecx = r.ecx.wrapping_sub(1);
                output = output.wrapping_add(2);
            }
            r.eax = (r.eax << 16) | (r.eax & 0xFFFF);
            let tail = r.ecx & 1;
            r.ecx >>= 1;
            while r.ecx != 0 {
                put_word(output, 0, r.eax);
                output = output.wrapping_add(4);
                r.ecx -= 1;
            }
            if tail != 0 {
                (output as *mut u16).write_unaligned(r.eax as u16);
            }
        }
    }
});

register_port!(
    0x00913409,
    fc4p_00913409,
    release_guarded_service_value,
    "ReleaseGuardedServiceValue",
    0,
    |r| {
        // SAFETY: the guard initializes its retained service interface, released after the value operation.
        unsafe {
            if r.argument(0) != 0 {
                let mut guard = [r.ecx; 2];
                r.ecx = guard.as_mut_ptr() as u32;
                r.direct_call(0x0048_C9F0, &[value_release_guard() as u32], false);
                r.ecx = value_release_service() as u32;
                r.direct_call(0x0050_5190, &[r.argument(0)], false);
                r.ecx = guard[1];
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 8, &[]);
            }
        }
    }
);

/// Keep stored doubles as memory operands, including their signaling-NaN behavior.
macro_rules! stored_double_operation {
    ($name:ident, $instruction:literal) => {
        fn $name(value: crate::x87::F80, operand: f64) -> crate::x87::F80 {
            let mut output = crate::x87::F80::ZERO;
            // SAFETY: the operands are live and the one temporary x87 register is popped.
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_double_operation!(add_stored_double, "fadd qword ptr [{operand}]");
stored_double_operation!(subtract_stored_double, "fsub qword ptr [{operand}]");
stored_double_operation!(multiply_stored_double, "fmul qword ptr [{operand}]");

port! {
    /// Average three samples, scale by upper minus lower, and add lower. The first two samples are stored as
    /// doubles before the next call. Every later intermediate retains x87 precision and exponent range.
    #[port(address = 0x0091_386B, owner = "shared", name = "ThreeSampleRandomRange", naked)]
    pub unsafe extern "C" fn fc4p_0091386b() {
        core::arch::naked_asm!(
            "sub esp, 12", "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "fld tbyte ptr [esp]", "add esp, 12", "ret 16",
            body = sym three_sample_random_range,
        );
    }
}

unsafe fn take_random_sample(r: &mut Registers, object: u32) -> crate::x87::F80 {
    let mut sample = crate::x87::F80::ZERO;
    // SAFETY: the sample entry returns one unrounded value in ST0, which is popped into its temporary.
    unsafe {
        r.ecx = object;
        r.direct_call(0x0091_372E, &[], false);
        core::arch::asm!("fstp tbyte ptr [{sample}]", sample = in(reg) &mut sample, out("st(0)") _);
    }
    sample
}

unsafe extern "thiscall" fn three_sample_random_range(frame: *mut Registers) {
    use crate::x87::F80;
    // SAFETY: the entry reserves twelve output bytes between its saved registers and original arguments.
    unsafe {
        let r = &mut *frame;
        let object = r.ecx;
        let mut stored = [0f64; 2];
        for value in &mut stored {
            *value = take_random_sample(r, object).to_f64();
        }
        let last = take_random_sample(r, object);
        let sum = add_stored_double(add_stored_double(last, stored[1]), stored[0]);
        let arguments = frame.cast::<u8>().add(28);
        let upper = arguments.add(8).cast::<f64>().read_unaligned();
        let lower = arguments.cast::<f64>().read_unaligned();
        let range = subtract_stored_double(F80::from_f64(upper), lower);
        let mut product = F80::ZERO;
        core::arch::asm!(
            "fld tbyte ptr [{sum}]", "fld tbyte ptr [{range}]", "fmulp st(1), st(0)",
            "fstp tbyte ptr [{product}]",
            sum = in(reg) &sum, range = in(reg) &range, product = in(reg) &mut product,
            out("st(0)") _, out("st(1)") _,
        );
        let result = add_stored_double(multiply_stored_double(product, three_sample_scale().read()), lower);
        frame.cast::<u8>().add(12).cast::<F80>().write_unaligned(result);
    }
}

// Range helpers use byte addresses: word ranges advance by two, record ranges by 28,
// and hash nodes keep their next link at zero and their value at four.
layout! {
    /// The shared hash table's bucket vector starts at four.
    struct SharedHashBuckets {
        0x04 => begin / set_begin: u32,
        0x08 => end / set_end: u32,
    }
}
layout! {
    /// A hash iterator keeps its current node and table in two adjacent words.
    struct SharedHashIterator {
        0x00 => node / set_node: u32,
        0x04 => table / set_table: u32,
    }
}
layout! {
    /// The enable flag protects a vector of 28-byte records; its lock member starts at 44.
    struct GuardedRecordVector {
        0x08 => enabled / set_enabled: u8,
        0x10 => begin / set_begin: u32,
        0x14 => end / set_end: u32,
    }
}
layout! {
    /// The stream lock starts at C; embedded stream interfaces start at 2C and 40.
    struct GuardedStreamFields {
        0x1C => mode / set_mode: u32,
        0x24 => index / set_index: u32,
        0xC0 => base_offset / set_base_offset: u32,
        0x144 => options / set_options: u32,
        0x158 => dirty / set_dirty: u8,
    }
}
layout! {
    /// The title interface starts D8 bytes after the primary window interface.
    struct TitleWindowFields {
        0x00 => vtable / set_vtable: u32,
        0xD8 => title_vtable / set_title_vtable: u32,
        0xDC => first_flag / set_first_flag: u8,
        0xDD => second_flag / set_second_flag: u8,
        0xE0 => first_value / set_first_value: u32,
        0xE4 => second_value / set_second_value: u32,
        0xE8 => third_value / set_third_value: u32,
        0xEC => fourth_value / set_fourth_value: u32,
    }
}
layout! {
    /// Alignment fields are measured from the secondary title interface.
    struct TitleAlignmentFields {
        0x2C => alignment / set_alignment: u32,
        0x38 => group / set_group: u32,
    }
}
game_fn! { #[allow(dead_code)] fn copy_record_range(first: u32, last: u32, output: u32, tag: u32, mode: u32) -> u32 = "C" 0x0090_91B2; }
game_fn! { #[allow(dead_code)] fn move_record_range(first: u32, last: u32, output: u32, tag: u32) -> u32 = "C" 0x0093_352F; }
game_fn! { #[allow(dead_code)] fn construct_retained_copy(this: *mut u8, source: u32) -> u32 = "thiscall" 0x0094_0627; }
game_fn! { #[allow(dead_code)] fn reset_node_value(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0093_AB29; }
game_fn! { #[allow(dead_code)] fn copy_pair(destination: u32, source: u32) -> u32 = "C" 0x0099_7792; }
game_fn! { #[allow(dead_code)] fn copy_hash_value(this: *mut u8, source: u32) -> u32 = "thiscall" 0x0096_E78B; }
game_fn! { #[allow(dead_code)] fn initialize_tree_tags(this: *mut u8, first: u32, second: u32) -> u32 = "thiscall" 0x0093_7FAD; }
game_fn! { #[allow(dead_code)] fn insert_unique_tree_value(this: *mut u8, output: u32, value: u32) -> u32 = "thiscall" 0x0096_F12D; }
game_fn! { #[allow(dead_code)] fn initialize_pointer_vector(this: *mut u8, count: u32, tag: u32) -> u32 = "thiscall" 0x009D_16D9; }
game_fn! { #[allow(dead_code)] fn fill_pointer_vector(destination: u32, count: u32, value: u32) -> u32 = "C" 0x0041_86F0; }
game_fn! { #[allow(dead_code)] fn compare_hash_value(this: *mut u8, value: u32) -> u32 = "thiscall" 0x0043_1180; }
game_fn! { #[allow(dead_code)] fn erase_hash_iterator(this: *mut u8, iterator: u32) -> u32 = "thiscall" 0x0097_C14C; }
game_fn! { #[allow(dead_code)] fn destroy_pointer_elements(first: u32, last: u32, tag: u32) -> u32 = "C" 0x0093_B776; }
game_fn! { #[allow(dead_code)] fn find_tree_key(this: *mut u8, key: u32) -> u32 = "thiscall" 0x0094_C8F5; }
game_fn! { #[allow(dead_code)] fn destroy_title_members(this: *mut u8) -> u32 = "thiscall" 0x0099_D2FE; }
game_fn! { #[allow(dead_code)] fn destroy_window_base(this: *mut u8) -> u32 = "thiscall" 0x0099_E1A2; }
game_fn! { #[allow(dead_code)] fn construct_window_base(this: *mut u8) -> u32 = "thiscall" 0x0099_D938; }
game_fn! { #[allow(dead_code)] fn erase_list_iterator(this: *mut u8, output: u32, position: u32) -> u32 = "thiscall" 0x0094_9259; }
game_fn! { #[allow(dead_code)] fn insert_tree_with_hint(this: *mut u8, output: u32, hint: u32, value: u32) -> u32 = "thiscall" 0x0098_1404; }

/// Hash bucket selection uses the CPU's unsigned division, including its zero-divisor fault.
fn bucket_remainder(hash: u32, count: u32) -> u32 {
    let remainder;
    // SAFETY: the instruction has no memory operands and declares both result registers.
    unsafe {
        core::arch::asm!(
            "div {count:e}", count = in(reg) count,
            inout("eax") hash => _, inout("edx") 0u32 => remainder,
            options(nomem, nostack),
        );
    }
    remainder
}

register_port!(
    0x00918731,
    fc4p_00918731,
    reverse_find_word,
    "ReverseFindWord",
    0,
    |r| {
        // SAFETY: the caller supplies an accessible word range and one writable output word.
        unsafe {
            r.ecx = r.argument(1);
            r.eax = r.argument(3);
            let begin = r.argument(2);
            r.edx = ((r.ecx.wrapping_sub(begin) as i32) >> 3) as u32;
            let mut found = None;
            while (r.edx as i32) > 0 {
                let previous = r.ecx.wrapping_sub(2);
                if (previous as *const u16).read_unaligned() == r.eax as u16 {
                    found = Some(r.ecx);
                    break;
                }
                r.ecx = previous;
                for _ in 0..3 {
                    let previous = r.ecx;
                    r.ecx = r.ecx.wrapping_sub(2);
                    if (r.ecx as *const u16).read_unaligned() == r.eax as u16 {
                        found = Some(previous);
                        break;
                    }
                }
                if found.is_some() {
                    break;
                }
                r.edx = r.edx.wrapping_sub(1);
            }
            if found.is_none() {
                r.edx = (((r.ecx.wrapping_sub(begin) as i32) >> 1) as u32).wrapping_sub(1);
                if r.edx != 0 {
                    r.edx = r.edx.wrapping_sub(1);
                    if r.edx != 0 {
                        r.edx = r.edx.wrapping_sub(1);
                    }
                }
                let remaining = (r.ecx.wrapping_sub(begin) as i32) >> 1;
                if (1..=3).contains(&remaining) {
                    for i in (1..=remaining).rev() {
                        if i > 1 {
                            r.edx = r.ecx.wrapping_sub(2);
                            if (r.edx as *const u16).read_unaligned() == r.eax as u16 {
                                found = Some(r.ecx);
                                break;
                            }
                            r.ecx = r.edx;
                        } else if (r.ecx.wrapping_sub(2) as *const u16).read_unaligned() == r.eax as u16 {
                            found = Some(r.ecx);
                        }
                    }
                }
            }
            r.eax = r.argument(0);
            put_word(r.eax, 0, found.unwrap_or(begin));
        }
    }
);

register_port!(
    0x009334f1,
    fc4p_009334f1,
    fill_seven_word_records,
    "FillSevenWordRecords",
    0,
    |r| {
        // SAFETY: each output record spans 28 bytes; the game's constructor receives the same source each time.
        unsafe {
            let mut output = r.argument(0);
            let mut count = r.argument(1);
            while count != 0 {
                let source = r.argument(2);
                r.direct_call(0x009A_7610, &[output, source], true);
                r.ecx = source;
                output = output.wrapping_add(28);
                count = count.wrapping_sub(1);
            }
            r.eax = output;
        }
    }
);

register_port!(
    0x0093352f,
    fc4p_0093352f,
    copy_records_forward,
    "CopyRecordsForward",
    0,
    |r| {
        // SAFETY: the helper receives the original range and a tag in its private stack storage.
        unsafe {
            let mut tag = r.ecx.to_ne_bytes();
            r.eax = tag.as_mut_ptr().add(3) as u32;
            r.direct_call(
                0x0090_91B2,
                &[r.argument(0), r.argument(1), r.argument(2), r.eax, 0],
                true,
            );
        }
    }
);

register_port!(
    0x0093354c,
    fc4p_0093354c,
    erase_seven_word_record,
    "EraseSevenWordRecord",
    4,
    |r| {
        // SAFETY: the record vector exposes its end at four and the selected position is in that range.
        unsafe {
            let vector = r.ecx;
            let position = r.argument(0);
            r.eax = word(vector, 4);
            r.ecx = position.wrapping_add(28);
            if r.ecx != r.eax {
                r.edx = (r.argument_pointer(0) as u32).wrapping_add(3);
                r.direct_call(0x0093_352F, &[r.ecx, r.eax, position, r.edx], true);
            }
            put_word(vector, 4, word(vector, 4).wrapping_sub(28));
            r.eax = position;
        }
    }
);

register_port!(
    0x009335ce,
    fc4p_009335ce,
    erase_seven_word_range,
    "EraseSevenWordRange",
    8,
    |r| {
        // SAFETY: the vector and both iterators are live; its end is replaced by the helper's result.
        unsafe {
            let vector = r.ecx;
            r.eax = (r.argument_pointer(0) as u32).wrapping_add(3);
            r.direct_call(
                0x0093_352F,
                &[r.argument(1), word(vector, 4), r.argument(0), r.eax],
                true,
            );
            put_word(vector, 4, r.eax);
            r.eax = r.argument(0);
        }
    }
);

register_port!(
    0x00940613,
    fc4p_00940613,
    construct_optional_retained_copy,
    "ConstructOptionalRetainedCopy",
    4,
    |r| {
        // SAFETY: a nonzero destination has space for the retained copy.
        unsafe {
            let destination = r.argument(0);
            if destination != 0 {
                let source = r.ecx;
                r.ecx = destination;
                r.direct_call(0x0094_0627, &[source], false);
            }
        }
    }
);

register_port!(
    0x00968c53,
    fc4p_00968c53,
    erase_retained_list_node,
    "EraseRetainedListNode",
    8,
    |r| {
        // SAFETY: the node's two links and embedded value belong to a live circular list.
        unsafe {
            let node = r.argument(1);
            let next = word(node, 0);
            r.eax = word(node, 4);
            put_word(r.eax, 0, next);
            r.ecx = node.wrapping_add(8);
            put_word(next, 4, r.eax);
            r.direct_call(0x0093_AB29, &[0], false);
            r.direct_call(0x0090_CF63, &[node], true);
            r.eax = r.argument(0);
            r.ecx = node;
            put_word(r.eax, 0, next);
        }
    }
);

register_port!(
    0x00968cb6,
    fc4p_00968cb6,
    allocate_pair_node,
    "AllocatePairNode",
    4,
    |r| {
        // SAFETY: the game's allocator supplies a 12-byte node and the pair constructor fills its value.
        unsafe {
            r.direct_call(0x0090_CF54, &[12], true);
            let node = r.eax;
            put_word(node, 0, 0);
            r.eax = node.wrapping_add(4);
            r.direct_call(0x0099_7792, &[r.eax, r.argument(0)], true);
            r.eax = node;
        }
    }
);

register_port!(
    0x0096edd3,
    fc4p_0096edd3,
    allocate_hash_value_node,
    "AllocateHashValueNode",
    4,
    |r| {
        // SAFETY: the game's allocator supplies the 24-byte node; its value begins four bytes later.
        unsafe {
            r.direct_call(0x0090_CF54, &[24], true);
            let node = r.eax;
            put_word(node, 0, 0);
            r.ecx = node.wrapping_add(4);
            if r.ecx != 0 {
                r.direct_call(0x0096_E78B, &[r.argument(0)], false);
            }
            r.eax = node;
        }
    }
);

register_port!(
    0x0096f57b,
    fc4p_0096f57b,
    construct_tree_with_shared_tag,
    "ConstructTreeWithSharedTag",
    0,
    |r| {
        // SAFETY: both constructor tags intentionally identify the same local byte.
        unsafe {
            let object = r.ecx;
            let mut tag = r.ecx.to_ne_bytes();
            r.eax = tag.as_mut_ptr().add(3) as u32;
            r.direct_call(0x0093_7FAD, &[r.eax, r.eax], false);
            r.eax = object;
        }
    }
);

register_port!(
    0x0096f594,
    fc4p_0096f594,
    insert_tree_pair_result,
    "InsertTreePairResult",
    8,
    |r| {
        // SAFETY: the helper fills a node word and flag byte; only these five bytes are copied out.
        unsafe {
            let mut result = [r.ecx; 2];
            r.eax = result.as_mut_ptr() as u32;
            r.direct_call(0x0096_F12D, &[r.eax, r.argument(1)], false);
            r.eax = r.argument(0);
            r.ecx = result[0];
            put_word(r.eax, 0, r.ecx);
            r.ecx = (r.ecx & !255) | (result[1] & 255);
            (r.eax.wrapping_add(4) as *mut u8).write(r.ecx as u8);
        }
    }
);

register_port!(
    0x0097c01d,
    fc4p_0097c01d,
    construct_filled_pointer_vector,
    "ConstructFilledPointerVector",
    12,
    |r| {
        // SAFETY: the initializer allocates the vector; the fill helper receives the original count and value.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x009D_16D9, &[r.argument(0), r.argument(2)], false);
            r.eax = word(object, 0);
            r.direct_call(0x0041_86F0, &[r.eax, r.argument(0), r.argument(1)], true);
            put_word(object, 4, r.eax);
            r.eax = object;
        }
    }
);

register_port!(
    0x0097c049,
    fc4p_0097c049,
    find_hash_value_node,
    "FindHashValueNode",
    4,
    |r| {
        // SAFETY: the hash vector has a nonzero bucket count and each linked node contains a readable value.
        unsafe {
            let object = r.ecx;
            let value = r.argument(0);
            let buckets = SharedHashBuckets::from_ptr(object as *mut u8);
            let count = ((buckets.end().wrapping_sub(buckets.begin()) as i32) >> 2) as u32;
            r.eax = word(value, 8);
            r.edx = bucket_remainder(r.eax, count);
            r.eax = word(object, 4);
            let mut node = word(r.eax, r.edx.wrapping_mul(4) as usize);
            while node != 0 {
                r.ecx = node.wrapping_add(4);
                r.direct_call(0x0043_1180, &[value], false);
                if r.eax as u8 != 0 {
                    break;
                }
                node = word(node, 0);
            }
            r.eax = node;
        }
    }
);

register_port!(
    0x0097c1ea,
    fc4p_0097c1ea,
    find_next_occupied_bucket,
    "FindNextOccupiedBucket",
    0,
    |r| {
        // SAFETY: the iterator identifies a live node and hash table, whose bucket vector is nonempty.
        unsafe {
            r.eax = word(r.ecx, 0);
            r.eax = word(r.eax, 12);
            let iterator = SharedHashIterator::from_ptr(r.ecx as *mut u8);
            let table = iterator.table();
            let count = ((word(table, 8).wrapping_sub(word(table, 4)) as i32) >> 2) as u32;
            r.edx = bucket_remainder(r.eax, count);
            r.eax = 0;
            loop {
                r.edx = r.edx.wrapping_add(1);
                if r.edx >= count {
                    break;
                }
                r.eax = word(r.ecx, 4);
                r.eax = word(r.eax, 4);
                r.eax = word(r.eax, r.edx.wrapping_mul(4) as usize);
                if r.eax != 0 {
                    break;
                }
            }
        }
    }
);

register_port!(
    0x0097c221,
    fc4p_0097c221,
    erase_hash_pair_iterator,
    "EraseHashPairIterator",
    8,
    |r| {
        // SAFETY: the copied iterator pair is private storage for the erase call.
        unsafe {
            let mut iterator = [r.argument(0), r.argument(1)];
            r.eax = iterator.as_mut_ptr() as u32;
            r.direct_call(0x0097_C14C, &[r.eax], false);
        }
    }
);

register_port!(
    0x00981543,
    fc4p_00981543,
    destroy_pointer_range,
    "DestroyPointerRange",
    0,
    |r| {
        // SAFETY: the original element destructor takes the accessible range and a private tag byte.
        unsafe {
            let mut tag = r.ecx.to_ne_bytes();
            r.eax = tag.as_mut_ptr().add(3) as u32;
            r.direct_call(0x0093_B776, &[r.argument(0), r.argument(1), r.eax], true);
        }
    }
);

register_port!(
    0x009855d0,
    fc4p_009855d0,
    blend_packed_color,
    "BlendPackedColor",
    0,
    |r| {
        // SAFETY: this cdecl helper reads three original argument words and has no pointer operands.
        unsafe {
            let source = r.argument(0);
            let destination = r.argument(1);
            let weight = r.argument(2);
            let high = destination & 0xFF00_FF00;
            r.edx = destination & 0x00FF_00FF;
            r.eax = ((source & 0x00FF_00FF)
                .wrapping_sub(r.edx)
                .wrapping_mul(weight)
                .wrapping_add(0x0080_0080)
                >> 8)
                .wrapping_add(r.edx);
            r.ecx = source >> 8;
            r.edx = (r.ecx & 0xFFFF_00FF)
                .wrapping_sub(high >> 8)
                .wrapping_mul(weight)
                .wrapping_add(high)
                .wrapping_add(0xFF80_0080);
            r.ecx = (r.ecx & 0x00FF_00FF)
                .wrapping_sub(high >> 8)
                .wrapping_mul(weight)
                .wrapping_add(high)
                .wrapping_add(0x0080_0080);
            r.eax = ((r.eax ^ r.edx) & 0x00FF_00FF) ^ r.ecx;
        }
    }
);

register_port!(
    0x009a94a6,
    fc4p_009a94a6,
    find_tree_value_pointer,
    "FindTreeValuePointer",
    8,
    |r| {
        // SAFETY: the tree search receives the original key word by address; the output is written only on a hit.
        unsafe {
            let tree = r.ecx;
            r.eax = r.argument_pointer(0) as u32;
            r.direct_call(0x0094_C8F5, &[r.eax], false);
            if r.eax != word(tree, 0) {
                r.ecx = r.argument(1);
                r.eax = r.eax.wrapping_add(20);
                put_word(r.ecx, 0, r.eax);
                r.eax = (r.eax & !255) | 1;
            } else {
                r.eax &= !255;
            }
        }
    }
);

register_port!(
    0x009cd1b2,
    fc4p_009cd1b2,
    destroy_title_window,
    "DestroyTitleWindow",
    0,
    |r| {
        // SAFETY: the live title window has primary and secondary tables and both base destructors.
        unsafe {
            let object = r.ecx;
            let title = TitleWindowFields::from_ptr(object as *mut u8);
            title.set_vtable(0x00AE_20A0);
            title.set_title_vtable(0x00AE_2050);
            r.direct_call(0x0099_D2FE, &[], false);
            r.ecx = object;
            r.direct_call(0x0099_E1A2, &[], false);
        }
    }
);

register_port!(
    0x009cd842,
    fc4p_009cd842,
    construct_title_window,
    "ConstructTitleWindow",
    0,
    |r| {
        // SAFETY: the base constructor initializes the live window; the title interface occupies D8..EF.
        unsafe {
            let object = r.ecx;
            r.direct_call(0x0099_D938, &[], false);
            put_word(object, 0xD8, 0x00AE_2300);
            put_word(object, 0xD8, 0x00AE_2050);
            (object.wrapping_add(0xDC) as *mut u8).write(0);
            (object.wrapping_add(0xDD) as *mut u8).write(0);
            for offset in [0xE0, 0xE4, 0xE8, 0xEC] {
                put_word(object, offset, 0);
            }
            put_word(object, 0, 0x00AE_20A0);
            r.eax = object;
        }
    }
);

port! {
    /// Keep the iterator result four bytes below entry ESP, including when its address is returned in EAX.
    #[port(address = 0x009D_6185, owner = "shared", name = "EraseFirstListNode", naked)]
    pub unsafe extern "C" fn fc4p_009d6185() {
        core::arch::naked_asm!(
            "sub esp, 4", "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "add esp, 4", "ret",
            body = sym erase_first_list_node,
        );
    }
}

unsafe extern "thiscall" fn erase_first_list_node(frame: *mut Registers) {
    // SAFETY: the entry reserves one original result word immediately above its register frame.
    unsafe {
        let r = &mut *frame;
        let output = frame.cast::<u8>().add(12) as u32;
        put_word(output, 0, r.ecx);
        r.edx = word(word(r.ecx, 0), 0);
        r.eax = output;
        r.direct_call(0x0094_9259, &[output, r.edx], false);
        r.ecx = word(output, 0);
    }
}

register_port!(
    0x009d74d9,
    fc4p_009d74d9,
    append_list_value,
    "AppendListValue",
    4,
    |r| {
        // SAFETY: insertion receives the original value word as both input and writable iterator output.
        unsafe {
            let value = r.argument(0);
            r.edx = word(word(r.ecx, 0), 0);
            r.eax = r.argument_pointer(0) as u32;
            r.direct_call(0x0089_7C8D, &[r.eax, r.edx, value], false);
        }
    }
);

register_port!(
    0x009dc50a,
    fc4p_009dc50a,
    insert_tree_hint_result,
    "InsertTreeHintResult",
    12,
    |r| {
        // SAFETY: the tree helper receives the original output, hint and value, and owns output initialization.
        unsafe {
            r.edx = r.argument(1);
            let hint = r.edx;
            let value = r.argument(2);
            let mut argument_word = hint;
            r.eax = &mut argument_word as *mut u32 as u32;
            r.direct_call(0x0098_1404, &[r.argument(0), hint, value], false);
            r.eax = r.argument(0);
        }
    }
);

game_fn! { #[allow(dead_code)] fn erase_record_position(this: *mut u8, position: u32) -> u32 = "thiscall" 0x0093_354C; }
game_fn! { #[allow(dead_code)] fn find_three_word_tree_key(this: *mut u8, key: u32) -> u32 = "thiscall" 0x0096_EEA9; }
game_fn! { #[allow(dead_code)] fn insert_pair_copy(this: *mut u8, output: u32, value: u32) -> u32 = "thiscall" 0x0096_F594; }
game_fn! { #[allow(dead_code)] fn refresh_title_style(this: *mut u8) -> u32 = "thiscall" 0x009C_1E6D; }

unsafe fn acquire_member_guard(r: &mut Registers, object: u32, offset: u32, guard: &mut [u32; 2]) {
    // SAFETY: the enclosing port supplies the live lock member and the guard's two stack words.
    unsafe {
        r.eax = object.wrapping_add(offset);
        r.ecx = guard.as_mut_ptr() as u32;
        r.direct_call(0x0048_C9F0, &[r.eax], false);
    }
}

unsafe fn release_member_guard(r: &mut Registers, guard: &[u32; 2]) {
    // SAFETY: the guard constructor fills its second word with the retained lock interface.
    unsafe {
        r.ecx = guard[1];
        r.eax = word(r.ecx, 0);
        r.virtual_call(r.ecx, 8, &[]);
    }
}

register_port!(
    0x0093357b,
    fc4p_0093357b,
    remove_guarded_record_key,
    "RemoveGuardedRecordKey",
    4,
    |r| {
        // SAFETY: the enabled object owns a record vector at 10 and lock at 44; records are 28 bytes.
        unsafe {
            let object = r.ecx;
            let mut removed = false;
            let records = GuardedRecordVector::from_ptr(object as *mut u8);
            if records.enabled() != 0 {
                let mut guard = [r.ecx; 2];
                acquire_member_guard(r, object, 0x44, &mut guard);
                r.ecx = object.wrapping_add(0x10);
                r.eax = word(r.ecx, 0);
                let end = word(object, 0x14);
                while r.eax != end {
                    r.edx = word(r.eax, 0);
                    if r.edx == r.argument(0) {
                        r.direct_call(0x0093_354C, &[r.eax], false);
                        removed = true;
                        break;
                    }
                    r.eax = r.eax.wrapping_add(28);
                }
                release_member_guard(r, &guard);
            }
            r.eax = (r.eax & !255) | u32::from(removed);
        }
    }
);

register_port!(
    0x0096dff0,
    fc4p_0096dff0,
    set_stream_size_when_closed,
    "SetStreamSizeWhenClosed",
    4,
    |r| {
        // SAFETY: the object owns its lock at C, access flags at 1C and embedded stream at 2C.
        unsafe {
            let object = r.ecx;
            let mut guard = [r.ecx; 2];
            acquire_member_guard(r, object, 0xC, &mut guard);
            let allowed = (object.wrapping_add(0x1C) as *const u8).read() & 3 == 0;
            if allowed {
                r.ecx = object.wrapping_add(0x2C);
                r.eax = word(r.ecx, 0);
                r.virtual_call(r.ecx, 0x34, &[r.argument(0)]);
            }
            release_member_guard(r, &guard);
            r.eax = (r.eax & !255) | u32::from(allowed);
        }
    }
);

register_port!(
    0x0096e15f,
    fc4p_0096e15f,
    set_guarded_stream_options,
    "SetGuardedStreamOptions",
    4,
    |r| {
        // SAFETY: the lock protects the options word at 144; the full caller word is stored unchanged.
        unsafe {
            let object = r.ecx;
            let mut guard = [r.ecx; 2];
            acquire_member_guard(r, object, 0xC, &mut guard);
            r.eax = r.argument(0);
            GuardedStreamFields::from_ptr(object as *mut u8).set_options(r.eax);
            release_member_guard(r, &guard);
            r.eax = (r.eax & !255) | 1;
        }
    }
);

unsafe fn transfer_at_stream_offset(r: &mut Registers, write: bool) {
    // SAFETY: the live stream has its lock at C, mode at 1C, embedded stream at 40 and base offset at C0.
    unsafe {
        let object = r.ecx;
        let mut guard = [r.ecx; 2];
        acquire_member_guard(r, object, 0xC, &mut guard);
        let permitted = if write {
            r.eax = word(object, 0x1C);
            r.eax & 2 != 0
        } else {
            (object.wrapping_add(0x1C) as *const u8).read() & 3 != 0
        };
        let mut success = false;
        if permitted {
            let stream = object.wrapping_add(0x40);
            r.eax = word(stream, 0);
            r.ecx = word(object, 0xC0).wrapping_add(r.argument(1));
            let position = r.ecx;
            r.ecx = stream;
            r.virtual_call(stream, 0x30, &[position]);
            r.ecx = word(object, 0xC0).wrapping_add(r.argument(1));
            if r.eax == r.ecx {
                r.eax = word(stream, 0);
                r.ecx = stream;
                r.virtual_call(stream, if write { 0x40 } else { 0x38 }, &[r.argument(0), r.argument(2)]);
                success = r.eax as u8 != 0;
            }
        }
        release_member_guard(r, &guard);
        r.eax = (r.eax & !255) | u32::from(success);
    }
}

register_port!(
    0x0096e1e5,
    fc4p_0096e1e5,
    read_guarded_stream_at_offset,
    "ReadGuardedStreamAtOffset",
    12,
    |r| {
        // SAFETY: this is the read form of the guarded transfer.
        unsafe {
            transfer_at_stream_offset(r, false);
        }
    }
);

register_port!(
    0x0096e2a7,
    fc4p_0096e2a7,
    write_guarded_stream_at_offset,
    "WriteGuardedStreamAtOffset",
    12,
    |r| {
        // SAFETY: this is the write form of the guarded transfer.
        unsafe {
            transfer_at_stream_offset(r, true);
        }
    }
);

register_port!(
    0x0096f7cd,
    fc4p_0096f7cd,
    lookup_guarded_record_size,
    "LookupGuardedRecordSize",
    4,
    |r| {
        // SAFETY: the lock protects the tree at 24; a found node stores the result word at 14.
        unsafe {
            let object = r.ecx;
            let mut guard = [r.ecx; 2];
            acquire_member_guard(r, object, 0xC, &mut guard);
            let mut result = u32::MAX;
            if (object.wrapping_add(0x1C) as *const u8).read() & 3 != 0 {
                r.eax = r.argument(0);
                let source = r.eax;
                r.ecx = word(source, 0);
                let first = r.ecx;
                r.ecx = word(source, 4);
                r.eax = word(source, 8);
                let mut key = [first, r.ecx, r.eax];
                r.eax = key.as_mut_ptr() as u32;
                r.ecx = word(object, 0x24);
                r.direct_call(0x0096_EEA9, &[r.eax], false);
                if r.eax != 0 {
                    result = word(r.eax, 0x14);
                }
            }
            release_member_guard(r, &guard);
            r.eax = result;
        }
    }
);

port! {
    /// Retain the result at entry ESP minus 20 and the range at entry ESP minus 12. The result address's
    /// upper bytes can remain in EAX after the success byte is set.
    #[port(address = 0x0096_FDE2, owner = "shared", name = "RememberNonemptyStreamRange", naked)]
    pub unsafe extern "C" fn fc4p_0096fde2() {
        core::arch::naked_asm!(
            "sub esp, 20", "push edx", "push ecx", "push eax", "mov ecx, esp", "call {body}",
            "pop eax", "pop ecx", "pop edx", "add esp, 20", "ret 8",
            body = sym remember_nonempty_stream_range,
        );
    }
}

unsafe extern "thiscall" fn remember_nonempty_stream_range(frame: *mut Registers) {
    // SAFETY: the entry reserves the original result and range slots above its saved registers.
    unsafe {
        let r = &mut *frame;
        let object = r.ecx;
        let arguments = frame.cast::<u8>().add(36).cast::<u32>();
        r.ecx = arguments.read_unaligned();
        if r.ecx != 0 {
            r.eax = arguments.add(1).read_unaligned();
            if r.eax != 0 {
                let pair = frame.cast::<u8>().add(20) as u32;
                put_word(pair, 4, r.eax);
                put_word(pair, 0, r.ecx);
                r.eax = frame.cast::<u8>().add(12) as u32;
                r.ecx = object.wrapping_add(0x14C);
                r.direct_call(0x0096_F594, &[r.eax, pair], false);
                (object.wrapping_add(0x158) as *mut u8).write(1);
            }
        }
        r.eax = (r.eax & !255) | 1;
    }
}

register_port!(
    0x00975929,
    fc4p_00975929,
    initialize_guarded_directory,
    "InitializeGuardedDirectory",
    12,
    |r| {
        // SAFETY: the directory lock is at eight and slot A0 performs initialization with the three original words.
        unsafe {
            let object = r.ecx;
            let mut guard = [r.ecx; 2];
            acquire_member_guard(r, object, 8, &mut guard);
            r.eax = word(object, 0);
            r.ecx = object;
            r.virtual_call(object, 0xA0, &[r.argument(0), r.argument(1), r.argument(2)]);
            let result = r.eax & 255;
            release_member_guard(r, &guard);
            r.eax = (r.eax & !255) | result;
        }
    }
);

register_port!(
    0x009c213e,
    fc4p_009c213e,
    set_title_alignment,
    "SetTitleAlignment",
    4,
    |r| {
        // SAFETY: the title interface has alignment at 2C and its three-way group at 38; the primary base is D8 earlier.
        unsafe {
            r.eax = r.argument(0);
            let alignment = TitleAlignmentFields::from_ptr(r.ecx as *mut u8);
            if r.eax != alignment.alignment() {
                r.edx = 2;
                if r.eax > 8 {
                    r.eax &= !255;
                    return;
                }
                alignment.set_group(
                    if r.eax <= 2 {
                        0
                    } else if r.eax <= 5 {
                        1
                    } else {
                        2
                    },
                );
                let base = r.ecx.wrapping_sub(0xD8);
                alignment.set_alignment(r.eax);
                r.ecx = base;
                r.direct_call(0x009C_1E6D, &[], false);
                r.eax = word(base, 0);
                r.ecx = base;
                r.virtual_call(base, 0x170, &[]);
            }
            r.eax = (r.eax & !255) | 1;
        }
    }
);

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00918731::PORT,
    fc4p_009334f1::PORT,
    fc4p_0093352f::PORT,
    fc4p_0093354c::PORT,
    fc4p_0093357b::PORT,
    fc4p_009335ce::PORT,
    fc4p_00940613::PORT,
    fc4p_00968c53::PORT,
    fc4p_00968cb6::PORT,
    fc4p_0096dff0::PORT,
    fc4p_0096e15f::PORT,
    fc4p_0096e1e5::PORT,
    fc4p_0096e2a7::PORT,
    fc4p_0096edd3::PORT,
    fc4p_0096f57b::PORT,
    fc4p_0096f594::PORT,
    fc4p_0096f7cd::PORT,
    fc4p_0096fde2::PORT,
    fc4p_00975929::PORT,
    fc4p_0097c01d::PORT,
    fc4p_0097c049::PORT,
    fc4p_0097c1ea::PORT,
    fc4p_0097c221::PORT,
    fc4p_00981543::PORT,
    fc4p_009855d0::PORT,
    fc4p_009a94a6::PORT,
    fc4p_009c213e::PORT,
    fc4p_009cd1b2::PORT,
    fc4p_009cd842::PORT,
    fc4p_009d6185::PORT,
    fc4p_009d74d9::PORT,
    fc4p_009dc50a::PORT,
    fc4p_008f6e80::PORT,
    fc4p_008ff785::PORT,
    fc4p_00900357::PORT,
    fc4p_009086b7::PORT,
    fc4p_0090990f::PORT,
    fc4p_0090b9e7::PORT,
    fc4p_0090cf9a::PORT,
    fc4p_0090d8e0::PORT,
    fc4p_0090d91c::PORT,
    fc4p_0090dc0f::PORT,
    fc4p_0090dc3d::PORT,
    fc4p_0090df7c::PORT,
    fc4p_0090e8b6::PORT,
    fc4p_0090ffc8::PORT,
    fc4p_00913409::PORT,
    fc4p_0091386b::PORT,
    fc4p_00910128::PORT,
    fc4p_0091030f::PORT,
    fc4p_0091036d::PORT,
    fc4p_009108b4::PORT,
    fc4p_009108ed::PORT,
    fc4p_0097c081::PORT,
    fc4p_00908902::PORT,
    fc4p_0090b2f6::PORT,
    fc4p_0090cff9::PORT,
    fc4p_0090d031::PORT,
    fc4p_0090ddd5::PORT,
    fc4p_009133ab::PORT,
    fc4p_0093de49::PORT,
    fc4p_00940627::PORT,
    fc4p_0094065e::PORT,
    fc4p_00940bc5::PORT,
    fc4p_00940db5::PORT,
    fc4p_00943a0e::PORT,
    fc4p_009446e4::PORT,
    fc4p_00945f24::PORT,
    fc4p_0094909a::PORT,
    fc4p_0096df8d::PORT,
    fc4p_0096e78b::PORT,
    fc4p_0096edf7::PORT,
    fc4p_0096f290::PORT,
    fc4p_0096f5b7::PORT,
    fc4p_0097005c::PORT,
    fc4p_009756f6::PORT,
    fc4p_0098129c::PORT,
    fc4p_00982d95::PORT,
    fc4p_00991e90::PORT,
    fc4p_009a841f::PORT,
    fc4p_009a84a0::PORT,
    fc4p_009a8947::PORT,
    fc4p_009a9427::PORT,
    fc4p_009abb63::PORT,
    fc4p_009b3537::PORT,
    fc4p_009bc8b8::PORT,
    fc4p_009be9af::PORT,
    fc4p_009db1a1::PORT,
    fc4p_009db998::PORT,
    fc4p_00a6d999::PORT,
    fc4p_008fe5ac::PORT,
    fc4p_009086ac::PORT,
    fc4p_0090dc2f::PORT,
    fc4p_0090e230::PORT,
    fc4p_009100de::PORT,
    fc4p_009103b0::PORT,
    fc4p_0091559a::PORT,
    fc4p_00920bf0::PORT,
    fc4p_0092973b::PORT,
    fc4p_0092b33a::PORT,
    fc4p_009334b1::PORT,
    fc4p_009334c2::PORT,
    fc4p_00934742::PORT,
    fc4p_0093474a::PORT,
    fc4p_009359bd::PORT,
    fc4p_009377c9::PORT,
    fc4p_009378bc::PORT,
    fc4p_00937ef1::PORT,
    fc4p_0093877e::PORT,
    fc4p_0093e552::PORT,
    fc4p_0093e55a::PORT,
    fc4p_0093e5fa::PORT,
    fc4p_00943917::PORT,
    fc4p_00945942::PORT,
    fc4p_00949ade::PORT,
    fc4p_00949b73::PORT,
    fc4p_0094a5ea::PORT,
    fc4p_0094bc3f::PORT,
    fc4p_0094c4b1::PORT,
    fc4p_0094d34f::PORT,
    fc4p_0095ba6f::PORT,
    fc4p_0095ba81::PORT,
    fc4p_0095ba8c::PORT,
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
    fc4p_0097569c::PORT,
    fc4p_00975a70::PORT,
    fc4p_009779c9::PORT,
    fc4p_0097ce34::PORT,
    fc4p_00989960::PORT,
    fc4p_00990010::PORT,
    fc4p_00991e80::PORT,
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
    fc4p_009c3286::PORT,
    fc4p_009c32ac::PORT,
    fc4p_009c32b0::PORT,
    fc4p_009c3eb7::PORT,
    fc4p_009c3f8f::PORT,
    fc4p_009c409f::PORT,
    fc4p_009c40b5::PORT,
    fc4p_009c40c0::PORT,
    fc4p_009c40c8::PORT,
    fc4p_009cc22d::PORT,
    fc4p_009cede1::PORT,
    fc4p_009d41a1::PORT,
    fc4p_009d4714::PORT,
    fc4p_009d6090::PORT,
    fc4p_009d6098::PORT,
    fc4p_009d6608::PORT,
    fc4p_009d6610::PORT,
    fc4p_009d8564::PORT,
    fc4p_009dae69::PORT,
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
        arguments: Vec<u32>,
    }

    #[derive(Default)]
    struct State {
        calls: Vec<VirtualCall>,
        direct_targets: Vec<u32>,
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

    unsafe fn observe(frame: *mut Registers, argument_count: usize) {
        // SAFETY: the fake entry supplies its saved registers and original argument words.
        unsafe {
            let r = &*frame;
            STATE.with(|s| {
                let mut s = s.borrow_mut();
                s.calls.push(VirtualCall {
                    this: r.ecx,
                    eax: r.eax,
                    edx: r.edx,
                    arguments: (0..argument_count).map(|i| r.argument(i)).collect(),
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
        unsafe { observe(frame, 0) };
    }

    unsafe extern "thiscall" fn observe_one(frame: *mut Registers) {
        // SAFETY: the fake's register frame, with one stack argument.
        unsafe { observe(frame, 1) };
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
            arguments: argument.into_iter().collect(),
        }
    }

    unsafe extern "thiscall" fn observe_two(frame: *mut Registers) {
        // SAFETY: the fake's register frame, with two stack arguments.
        unsafe { observe(frame, 2) };
    }

    #[unsafe(naked)]
    unsafe extern "C" fn fake_two() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}", "add esp, 12",
            "mov eax, 0xa1b2c3d4", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret 8",
            sym observe_two,
        );
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
        x87_input: [u8; 10],
        x87_output: [u8; 10],
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
            x87_input: [0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x81, 0xFF, 0x3F],
            x87_output: [0; 10],
        };
        // SAFETY: each test supplies the export's convention and live storage. The probe preserves
        // EDI, and the existing x87 value must survive the port and is popped after the probe.
        unsafe {
            core::arch::asm!(
                "fld tbyte ptr [edi + 68]", "push edi", "call {probe}", "add esp, 4",
                "fstp tbyte ptr [edi + 78]",
                in("edi") &mut call,
                probe = sym probe, clobber_abi("C"),
                out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
            );
        }
        assert_eq!(call.x87_output, call.x87_input, "existing x87 value");
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

    #[test]
    fn tail_adapters_forward_exact_addresses_arguments_and_all_return_registers() {
        for (f, target, adjustment, count) in [
            (fc4p_00934742 as *const () as usize, 0x0093_784B, -0x1C, 0),
            (fc4p_0093474a as *const () as usize, 0x0093_7850, -0x1C, 0),
            (fc4p_009359bd as *const () as usize, 0x0094_C47C, -4, 0),
            (fc4p_0093e552 as *const () as usize, 0x0093_A897, -4, 0),
            (fc4p_0093e55a as *const () as usize, 0x0093_A89E, -4, 0),
            (fc4p_0093e5fa as *const () as usize, 0x0093_A89E, -0x1C, 0),
            (fc4p_0095ba81 as *const () as usize, 0x008F_B729, -0xD8, 0),
            (fc4p_0095ba8c as *const () as usize, 0x008F_B72E, -0xD8, 0),
            (fc4p_00975a70 as *const () as usize, 0x0097_59CE, 4, 0),
            (fc4p_00991e80 as *const () as usize, 0x0099_1E90, -4, 1),
            (fc4p_009c409f as *const () as usize, 0x009C_38FD, -0x124, 2),
            (fc4p_009c40b5 as *const () as usize, 0x008F_B72E, -0x124, 0),
            (fc4p_009c40c0 as *const () as usize, 0x0095_BA6F, -0x4C, 0),
            (fc4p_009c40c8 as *const () as usize, 0x009C_3EAD, -0x4C, 1),
            (fc4p_009cc22d as *const () as usize, 0x0091_DD89, 8, 0),
            (fc4p_009d6090 as *const () as usize, 0x0094_C47C, 4, 0),
            (fc4p_009d6098 as *const () as usize, 0x0099_B7A3, 4, 0),
            (fc4p_009d6608 as *const () as usize, 0x009D_6090, -4, 0),
            (fc4p_009d6610 as *const () as usize, 0x009D_6098, -4, 0),
            (fc4p_009dae69 as *const () as usize, 0x009D_AF24, -4, 2),
        ] {
            reset();
            let fake = match count {
                0 => fake_zero as *const () as usize,
                1 => fake_one as *const () as usize,
                _ => fake_two as *const () as usize,
            };
            let _callee = crate::testing::fake(target, fake);
            for this in [0u32, 1, 4, 0x124, 0x1234_5678, u32::MAX] {
                let args = [0x8765_4321, 0xFEDC_BA98];
                assert_eq!(run(f, this, &args[..count]), [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                assert_eq!(
                    calls(),
                    [VirtualCall {
                        this: this.wrapping_add(adjustment as u32),
                        eax: ENTRY_EAX,
                        edx: ENTRY_EDX,
                        arguments: args[..count].to_vec(),
                    }]
                );
            }
        }
    }

    #[test]
    fn destructor_adapters_restore_the_table_before_forwarding_the_member() {
        for (f, target, table, adjustment) in [
            (fc4p_0090dc2f as *const () as usize, 0x0093_BAEF, 0x00AC_E650u32, 4),
            (fc4p_0097569c as *const () as usize, 0x0093_0B02, 0x00AD_8CA0u32, 8),
        ] {
            reset();
            let _callee = crate::testing::fake(target, fake_zero as *const () as usize);
            let mut object = [0xA5; 32];
            let mut expected = object;
            expected[..4].copy_from_slice(&table.to_le_bytes());
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 32)));
            assert_eq!(run(f, this, &[]), [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
            assert_eq!(calls(), [expected_call(this + adjustment, ENTRY_EAX, None)]);
            assert_eq!(object, expected);
            STATE.with(|s| assert_eq!(s.borrow().snapshots, [expected.to_vec()]));
        }
    }

    #[test]
    fn virtual_value_forwarder_returns_this_and_keeps_callee_ecx_and_edx() {
        for value in [0u32, 1, 0x8000_0000, u32::MAX] {
            reset();
            let mut vt = [0usize; 15];
            vt[0x38 / 4] = fake_one as *const () as usize;
            let mut object = [vt.as_ptr() as u32, 0xAAAA_AAAA];
            let before = object;
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_0090e230 as *const () as usize, this, &[value]),
                [this, RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(calls(), [expected_call(this, vt.as_ptr() as u32, Some(value))]);
            assert_eq!(object, before);
        }
    }

    #[test]
    fn scrollbar_limits_store_before_notifications_and_reload_the_window_table() {
        for (f, offset) in [
            (fc4p_009c3286 as *const () as usize, 0xE0),
            (fc4p_009c32b0 as *const () as usize, 0xE4),
        ] {
            for value in [0u32, 1, 0x8000_0000, u32::MAX] {
                reset();
                let mut first_vt = [0usize; 154];
                let mut second_vt = [0usize; 154];
                first_vt[0x264 / 4] = fake_zero as *const () as usize;
                second_vt[0x170 / 4] = fake_zero as *const () as usize;
                let mut object = [0xA5; 0x120];
                object[..4].copy_from_slice(&(first_vt.as_ptr() as u32).to_le_bytes());
                let mut before_second = object;
                before_second[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.watch = Some((this, 0x120));
                    s.mutation = Some((this, 0, second_vt.as_ptr() as u32));
                });
                assert_eq!(run(f, this + 0xD8, &[value]), [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                assert_eq!(
                    calls(),
                    [
                        expected_call(this, first_vt.as_ptr() as u32, None),
                        VirtualCall {
                            this,
                            eax: second_vt.as_ptr() as u32,
                            edx: RESULT_EDX,
                            arguments: vec![]
                        },
                    ]
                );
                let mut expected = before_second;
                expected[..4].copy_from_slice(&(second_vt.as_ptr() as u32).to_le_bytes());
                assert_eq!(object, expected);
                STATE.with(|s| assert_eq!(s.borrow().snapshots, [before_second.to_vec(), expected.to_vec()]));
            }
        }
    }

    #[test]
    fn paired_window_values_forward_in_order_and_set_only_the_result_low_byte() {
        reset();
        let mut first_vt = [0usize; 9];
        let mut second_vt = [0usize; 9];
        first_vt[0x18 / 4] = fake_one as *const () as usize;
        second_vt[0x20 / 4] = fake_one as *const () as usize;
        let mut object = [0xA5; 0x80];
        object[..4].copy_from_slice(&(first_vt.as_ptr() as u32).to_le_bytes());
        let mut expected = object;
        expected[..4].copy_from_slice(&(second_vt.as_ptr() as u32).to_le_bytes());
        let this = object.as_mut_ptr() as u32;
        STATE.with(|s| s.borrow_mut().mutation = Some((this, 0, second_vt.as_ptr() as u32)));
        assert_eq!(
            run(
                fc4p_009c3f8f as *const () as usize,
                this + 0x4C,
                &[0xFEDC_BA98, 0x8765_4321]
            ),
            [(RESULT_EAX & !255) | 1, RESULT_ECX, RESULT_EDX]
        );
        assert_eq!(
            calls(),
            [
                expected_call(this, first_vt.as_ptr() as u32, Some(0xFEDC_BA98)),
                VirtualCall {
                    this,
                    eax: second_vt.as_ptr() as u32,
                    edx: RESULT_EDX,
                    arguments: vec![0x8765_4321]
                },
            ]
        );
        assert_eq!(object, expected);
    }
    unsafe extern "thiscall" fn observe_five(frame: *mut Registers) {
        // SAFETY: the fake entry supplies five stack words.
        unsafe { observe(frame, 5) };
    }
    #[unsafe(naked)]
    unsafe extern "C" fn fake_five() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}", "add esp, 12",
            "mov eax, 0xa1b2c3d4", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret 20",
            sym observe_five,
        );
    }
    #[unsafe(naked)]
    unsafe extern "C" fn fake_other_type() {
        core::arch::naked_asm!(
            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {}", "add esp, 12",
            "mov eax, 0xa1b2c3d5", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret",
            sym observe_zero,
        );
    }

    #[test]
    fn default_message_dispatch_checks_the_full_id_and_forwards_five_words() {
        reset();
        let mut vt = [0usize; 5];
        vt[4] = fake_five as *const () as usize;
        let mut object = [vt.as_ptr() as u32, 1, 2, 3, 4, 0xFEDC_BA98];
        let before = object;
        let this = object.as_mut_ptr() as u32;
        for id in [0u32, 0x856D_DBAC, 0x856D_DBAD, 0x856D_DB00, u32::MAX] {
            let got = run(
                fc4p_0090b2f6 as *const () as usize,
                this,
                &[id, 0xABCD_1234, 0x8765_4321],
            );
            if id == 0x856D_DBAC {
                assert_eq!(got, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                assert_eq!(
                    calls(),
                    [VirtualCall {
                        this,
                        eax: id,
                        edx: before[5],
                        arguments: vec![id, 0xABCD_1234, 0x8765_4321, before[5], 0]
                    }]
                );
            } else {
                assert_eq!(got, [0x856D_DB00, this, ENTRY_EDX]);
                assert!(calls().is_empty());
            }
            assert_eq!(object, before);
        }
    }

    #[test]
    fn moving_list_nodes_preserves_rings_and_updates_active_links() {
        for n in 1..=4usize {
            for moving in 0..n {
                for active in 0..=n {
                    let mut nodes = [[0u32; 4]; 4];
                    let ptrs: [u32; 4] = core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32);
                    for i in 0..n {
                        nodes[i] = [ptrs[(i + 1) % n], ptrs[(i + n - 1) % n], 0xABCD_1234, 0x8000_0001];
                    }
                    let mut list = [
                        0x1111_1111,
                        ptrs[0],
                        if active == n { 0 } else { ptrs[active] },
                        0x2222_2222,
                    ];
                    let this = list.as_mut_ptr() as u32;
                    let mut expected = nodes;
                    let mut order: Vec<usize> = (0..n).filter(|i| *i != moving).collect();
                    if moving == 0 {
                        order.push(moving);
                    } else {
                        order.insert(0, moving);
                    }
                    for (j, &i) in order.iter().enumerate() {
                        expected[i][0] = ptrs[order[(j + 1) % n]];
                        expected[i][1] = ptrs[order[(j + n - 1) % n]];
                    }
                    // Moving the anchor keeps the original self-linking write order.
                    if moving == 0 && n > 1 {
                        expected[0][0] = ptrs[0];
                        expected[0][1] = ptrs[0];
                        expected[1][1] = ptrs[n - 1];
                        expected[n - 1][0] = ptrs[0];
                    }
                    expected[moving][3] = 0x4000_0001;
                    let current = if active == moving {
                        if n == 1 {
                            0
                        } else {
                            ptrs[(moving + n - 1) % n]
                        }
                    } else {
                        list[2]
                    };
                    let tail = if n == 1 {
                        ptrs[0]
                    } else {
                        ptrs[if moving == n - 1 { n - 2 } else { n - 1 }]
                    };
                    assert_eq!(
                        run(fc4p_0090cff9 as *const () as usize, this, &[ptrs[moving]]),
                        [ptrs[moving], ptrs[0], tail]
                    );
                    assert_eq!(nodes, expected);
                    assert_eq!(list, [0x1111_1111, ptrs[0], current, 0x2222_2222]);
                }
            }
        }
    }

    #[test]
    fn unlink_skips_singletons_and_releases_after_fixing_neighbors() {
        for singleton in [false, true] {
            for is_head in [false, true] {
                reset();
                let mut vt = [0usize; 4];
                vt[3] = fake_one as *const () as usize;
                let mut node = [0u32; 4];
                let mut previous = [0xABCD_1234u32; 4];
                let mut next = [0x9876_5432u32; 4];
                let p = node.as_mut_ptr() as u32;
                let prev = if singleton { p } else { previous.as_mut_ptr() as u32 };
                let succ = if singleton { p } else { next.as_mut_ptr() as u32 };
                node[0] = succ;
                node[1] = prev;
                let mut object = [vt.as_ptr() as u32, if is_head { p } else { succ }, 0x5555_5555];
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| s.borrow_mut().watch = Some((this, 12)));
                let got = run(fc4p_0090d031 as *const () as usize, this, &[p]);
                if singleton {
                    assert_eq!(got, [p, this, p]);
                    assert!(calls().is_empty());
                } else {
                    assert_eq!(got, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                    assert_eq!(previous[0], succ);
                    assert_eq!(next[1], prev);
                    assert_eq!(object[1], if is_head { prev } else { succ });
                    assert_eq!(
                        calls(),
                        [VirtualCall {
                            this,
                            eax: vt.as_ptr() as u32,
                            edx: p,
                            arguments: vec![p]
                        }]
                    );
                    STATE.with(|s| assert_eq!(&s.borrow().snapshots[0][4..8], &object[1].to_le_bytes()));
                }
                assert_eq!(node, [succ, prev, 0, 0]);
            }
        }
    }

    #[test]
    fn service_destructor_checks_null_combinations_and_reloads_fields() {
        for mask in 0..8u32 {
            reset();
            let mut vt = [0usize; 3];
            vt[2] = fake_zero as *const () as usize;
            let mut service = [vt.as_ptr() as u32];
            let p = service.as_mut_ptr() as u32;
            let mut object = [0xAAAA_AAAAu32; 12];
            for (i, offset) in [10, 9, 8].into_iter().enumerate() {
                object[offset] = if mask & (1 << i) != 0 { p } else { 0 };
            }
            let mut expected = object;
            expected[0] = 0x00AC_EC00;
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 48)));
            assert_eq!(
                run(fc4p_009133ab as *const () as usize, this, &[]),
                [
                    if mask == 0 { ENTRY_EAX } else { RESULT_EAX },
                    if mask & 4 != 0 { RESULT_ECX } else { 0 },
                    if mask == 0 { ENTRY_EDX } else { RESULT_EDX }
                ]
            );
            assert_eq!(object, expected);
            let trace = calls();
            assert_eq!(trace.len(), mask.count_ones() as usize);
            for (i, call) in trace.iter().enumerate() {
                assert_eq!(
                    call,
                    &VirtualCall {
                        this: p,
                        eax: vt.as_ptr() as u32,
                        edx: if i == 0 { ENTRY_EDX } else { RESULT_EDX },
                        arguments: vec![]
                    }
                );
            }
            STATE.with(|s| {
                for bytes in &s.borrow().snapshots {
                    assert_eq!(&bytes[..4], &0x00AC_EC00u32.to_le_bytes());
                }
            });
        }
        reset();
        let mut vt = [0usize; 3];
        vt[2] = fake_zero as *const () as usize;
        let mut service = [vt.as_ptr() as u32];
        let p = service.as_mut_ptr() as u32;
        let mut object = [0u32; 12];
        object[10] = p;
        object[9] = p;
        let this = object.as_mut_ptr() as u32;
        STATE.with(|s| s.borrow_mut().mutation = Some((this, 0x24, 0)));
        run(fc4p_009133ab as *const () as usize, this, &[]);
        assert_eq!(calls().len(), 1);
    }

    #[test]
    fn interface_copy_stores_before_retaining_and_accepts_self_copy() {
        for present in [false, true] {
            for self_copy in [false, true] {
                reset();
                let mut vt = [0usize; 2];
                vt[1] = fake_zero as *const () as usize;
                let mut service = [vt.as_ptr() as u32];
                let value = if present { service.as_mut_ptr() as u32 } else { 0 };
                let mut object = [0xAAAA_AAAA, value, 0xBBBB_BBBB];
                let mut source = [0xCCCC_CCCC, value];
                let this = object.as_mut_ptr() as u32;
                let src = if self_copy { this } else { source.as_mut_ptr() as u32 };
                STATE.with(|s| s.borrow_mut().watch = Some((this, 12)));
                assert_eq!(
                    run(fc4p_00940627 as *const () as usize, this, &[src]),
                    [
                        this,
                        if present { RESULT_ECX } else { 0 },
                        if present { RESULT_EDX } else { ENTRY_EDX }
                    ]
                );
                assert_eq!(object, [0x00AD_43D8, value, 0xBBBB_BBBB]);
                if present {
                    assert_eq!(calls(), [expected_call(value, vt.as_ptr() as u32, None)]);
                    STATE.with(|s| assert_eq!(&s.borrow().snapshots[0][4..8], &value.to_le_bytes()));
                } else {
                    assert!(calls().is_empty());
                }
            }
        }
    }

    #[test]
    fn type_assignment_handles_null_mismatches_and_value_upper_bytes() {
        for same in [false, true] {
            for null in [false, true] {
                reset();
                let mut vt = [0usize; 5];
                vt[4] = fake_zero as *const () as usize;
                let mut other_vt = vt;
                other_vt[4] = fake_other_type as *const () as usize;
                let mut source = [
                    if same {
                        vt.as_ptr() as u32
                    } else {
                        other_vt.as_ptr() as u32
                    },
                    0xFEDC_BA98,
                ];
                let mut object = [vt.as_ptr() as u32, 0xABCD_1234];
                let this = object.as_mut_ptr() as u32;
                let src = if null { 0 } else { source.as_mut_ptr() as u32 };
                let want = if null {
                    ENTRY_EAX & !255
                } else if same {
                    0xFEDC_BA01
                } else {
                    RESULT_EAX & !255
                };
                assert_eq!(
                    run(fc4p_00940db5 as *const () as usize, this, &[src]),
                    [
                        want,
                        if null { this } else { RESULT_ECX },
                        if null { ENTRY_EDX } else { RESULT_EDX }
                    ]
                );
                assert_eq!(object[1], if !null && same { source[1] } else { 0xABCD_1234 });
                let trace = calls();
                if !null {
                    assert_eq!(
                        trace,
                        [
                            expected_call(src, source[0], None),
                            VirtualCall {
                                this,
                                eax: vt.as_ptr() as u32,
                                edx: RESULT_EDX,
                                arguments: vec![]
                            }
                        ]
                    );
                } else {
                    assert!(trace.is_empty());
                }
            }
        }
    }

    #[test]
    fn buffer_release_reloads_owner_and_zeros_fields_between_calls() {
        reset();
        let mut vt = [0usize; 5];
        vt[4] = fake_one as *const () as usize;
        let mut second_vt = [0usize; 3];
        second_vt[2] = fake_zero as *const () as usize;
        let mut first = [vt.as_ptr() as u32];
        let mut second = [second_vt.as_ptr() as u32];
        let mut object = [
            0x1234_5678,
            0x8765_4321,
            u32::MAX,
            0xAAAA_AAAA,
            first.as_mut_ptr() as u32,
        ];
        let this = object.as_mut_ptr() as u32;
        let new_owner = second.as_mut_ptr() as u32;
        STATE.with(|s| {
            let mut s = s.borrow_mut();
            s.watch = Some((this, 20));
            s.mutation = Some((this, 0x10, new_owner));
        });
        assert_eq!(
            run(fc4p_009446e4 as *const () as usize, this, &[]),
            [RESULT_EAX, RESULT_ECX, RESULT_EDX]
        );
        assert_eq!(object, [0, 0, 0, 0xAAAA_AAAA, 0]);
        assert_eq!(
            calls(),
            [
                expected_call(first.as_mut_ptr() as u32, vt.as_ptr() as u32, Some(0x1234_5678)),
                VirtualCall {
                    this: new_owner,
                    eax: second_vt.as_ptr() as u32,
                    edx: RESULT_EDX,
                    arguments: vec![]
                }
            ]
        );
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(&s.snapshots[0][..4], &0x1234_5678u32.to_le_bytes());
            assert_eq!(&s.snapshots[1][..12], &[0; 12]);
            assert_eq!(&s.snapshots[1][16..20], &new_owner.to_le_bytes());
        });
    }

    #[test]
    fn retained_assignment_covers_null_identity_and_retain_before_release() {
        for old_present in [false, true] {
            for new_kind in 0..3 {
                reset();
                let mut vt = [0usize; 3];
                vt[1] = fake_zero as *const () as usize;
                vt[2] = fake_zero as *const () as usize;
                let mut old_service = [vt.as_ptr() as u32];
                let mut new_service = old_service;
                let old = if old_present {
                    old_service.as_mut_ptr() as u32
                } else {
                    0
                };
                let new = match new_kind {
                    0 => 0,
                    1 => old,
                    _ => new_service.as_mut_ptr() as u32,
                };
                let mut object = [old, 0xABCD_1234];
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| s.borrow_mut().watch = Some((this, 8)));
                let order: Vec<u32> = if old == new {
                    vec![]
                } else {
                    [new, old].into_iter().filter(|p| *p != 0).collect()
                };
                assert_eq!(
                    run(fc4p_0094909a as *const () as usize, this, &[new]),
                    [
                        this,
                        if order.is_empty() { this } else { RESULT_ECX },
                        if order.is_empty() { ENTRY_EDX } else { RESULT_EDX }
                    ]
                );
                assert_eq!(object, [new, 0xABCD_1234]);
                let trace = calls();
                assert_eq!(trace.len(), order.len());
                for (i, call) in trace.iter().enumerate() {
                    assert_eq!(
                        call,
                        &VirtualCall {
                            this: order[i],
                            eax: vt.as_ptr() as u32,
                            edx: if i == 0 { ENTRY_EDX } else { RESULT_EDX },
                            arguments: vec![]
                        }
                    );
                }
                STATE.with(|s| {
                    for (i, bytes) in s.borrow().snapshots.iter().enumerate() {
                        assert_eq!(&bytes[..4], &(if new != 0 && i == 0 { old } else { new }).to_le_bytes());
                    }
                });
            }
        }
    }

    #[test]
    fn record_copy_and_swap_keep_sequential_accesses_with_overlap() {
        for source in 0..6usize {
            for destination in 0..6usize {
                let mut data: [u32; 12] = core::array::from_fn(|i| 0xABCD_1200 | i as u32);
                let mut expected = data;
                for i in 0..5 {
                    expected[destination + i] = expected[source + i];
                }
                let this = data.as_mut_ptr().wrapping_add(destination) as u32;
                let src = data.as_mut_ptr().wrapping_add(source) as u32;
                assert_eq!(
                    run(fc4p_0096e78b as *const () as usize, this, &[src]),
                    [this, expected[destination + 4], expected[destination + 3]]
                );
                assert_eq!(data, expected);
                let mut edx = 0;
                for i in 0..3 {
                    let a = expected[destination + i];
                    let b = expected[source + i];
                    expected[destination + i] = b;
                    expected[source + i] = a;
                    edx = a;
                }
                assert_eq!(run(fc4p_0096edf7 as *const () as usize, this, &[src]), [src, this, edx]);
                assert_eq!(data, expected);
            }
        }
    }

    #[test]
    fn unsigned_tree_lookup_checks_empty_missing_equal_and_high_bit_keys() {
        let mut header = [0u32; 5];
        let sentinel = header.as_mut_ptr() as u32;
        let mut nodes = [[0u32; 5]; 5];
        let ptrs: [u32; 5] = core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32);
        let keys = [0, 1, 0x7FFF_FFFF, 0x8000_0000, u32::MAX];
        for i in 0..5 {
            nodes[i][4] = keys[i];
        }
        nodes[2][2] = ptrs[0];
        nodes[0][3] = ptrs[1];
        nodes[2][3] = ptrs[4];
        nodes[4][2] = ptrs[3];
        let mut object = [sentinel, 0xABCD_1234];
        let this = object.as_mut_ptr() as u32;
        for empty in [false, true] {
            header[1] = if empty { 0 } else { ptrs[2] };
            let before = nodes;
            let header_before = header;
            for mut key in [0, 1, 2, 0x7FFF_FFFE, 0x7FFF_FFFF, 0x8000_0000, 0x8000_0001, u32::MAX] {
                let found = if empty {
                    None
                } else {
                    keys.iter().position(|v| *v == key)
                };
                assert_eq!(
                    run(
                        fc4p_0098129c as *const () as usize,
                        this,
                        &[&mut key as *mut u32 as u32]
                    ),
                    [
                        found.map_or(sentinel, |i| ptrs[i]),
                        if empty { 0 } else { key },
                        sentinel
                    ]
                );
            }
            assert_eq!(nodes, before);
            assert_eq!(header, header_before);
        }
    }

    #[test]
    fn byte_accumulation_handles_empty_unaligned_and_long_inputs() {
        let mut data = [0xAAu8, 0, 1, 0x7F, 0x80, 0xFF, 0x12, 0x34, 0x56, 0x78, 0x9A, 0xBC];
        let before = data;
        for start in 0..data.len() {
            for n in 0..=data.len() - start {
                let ptr = data.as_mut_ptr().wrapping_add(start) as u32;
                let want = data[start..start + n]
                    .iter()
                    .fold(0u32, |sum, b| sum.wrapping_mul(256).wrapping_add(u32::from(*b)));
                assert_eq!(
                    invoke(
                        fc4p_00a6d999 as *const () as usize,
                        0x1234,
                        &[ptr, n as u32],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true
                    ),
                    [want, ptr + n as u32, 0]
                );
            }
        }
        assert_eq!(
            invoke(
                fc4p_00a6d999 as *const () as usize,
                0x1234,
                &[0, 0],
                ENTRY_EAX,
                ENTRY_EDX,
                true
            ),
            [0, 0, 0]
        );
        assert_eq!(data, before);
    }

    unsafe extern "stdcall" fn observe_direct(frame: *mut Registers, address: u32, count: usize) {
        STATE.with(|s| s.borrow_mut().direct_targets.push(address));
        // SAFETY: the fake supplies its original registers and argument count.
        unsafe {
            observe(frame, count);
        }
    }
    macro_rules! direct_fake {
        ($name:ident, $address:literal, $count:literal, $pop:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "push edx", "push ecx", "push eax", "mov eax, esp",
                    "push {count}", "push {address}", "push eax", "call {observe}", "add esp, 12",
                    "mov eax, 0xa1b2c3d4", "mov ecx, 0x4321abcd", "mov edx, 0xd4c3b2a1", "ret {pop}",
                    count = const $count, address = const $address, pop = const $pop, observe = sym observe_direct,
                );
            }
        };
    }
    direct_fake!(fake_hanging_destructor, 0x0090_86B7, 0, 0);
    direct_fake!(fake_container_destructor, 0x0090_DC2F, 0, 0);
    direct_fake!(fake_value_destructor, 0x0094_002A, 0, 0);
    direct_fake!(fake_adjusted_destructor, 0x0090_D964, 0, 0);
    direct_fake!(fake_text_destructor, 0x0041_4250, 0, 0);
    direct_fake!(fake_free_scalar, 0x005E_5620, 1, 0);
    direct_fake!(fake_free_array, 0x0090_CF63, 1, 0);
    direct_fake!(fake_allocate_array, 0x0090_CF54, 1, 0);
    direct_fake!(fake_bounds_base, 0x009A_B992, 0, 0);
    direct_fake!(fake_node_member, 0x0097_BF85, 1, 4);
    direct_fake!(fake_set_fill, 0x0099_BF1F, 1, 4);
    direct_fake!(fake_erase_branch, 0x009A_84A0, 1, 4);
    direct_fake!(fake_clear_tree, 0x009A_8947, 0, 0);
    direct_fake!(fake_list_destructor, 0x0096_EDBC, 0, 0);
    direct_fake!(fake_string_destructor, 0x0056_50A0, 0, 0);
    direct_fake!(fake_vector_range, 0x0098_1543, 2, 0);
    direct_fake!(fake_next_bucket, 0x0097_C1EA, 0, 0);
    direct_fake!(fake_hash_lookup, 0x0097_C049, 1, 4);
    direct_fake!(fake_hash_reserve, 0x0096_F614, 1, 4);
    direct_fake!(fake_hash_insert, 0x0096_F209, 2, 8);
    direct_fake!(fake_pointer_grow, 0x0097_3C0E, 2, 8);

    fn install_game(address: u32) -> crate::testing::Fake {
        let f = match address {
            0x0090_86B7 => fake_hanging_destructor as *const (),
            0x0090_DC2F => fake_container_destructor as *const (),
            0x0094_002A => fake_value_destructor as *const (),
            0x0090_D964 => fake_adjusted_destructor as *const (),
            0x0041_4250 => fake_text_destructor as *const (),
            0x005E_5620 => fake_free_scalar as *const (),
            0x0090_CF63 => fake_free_array as *const (),
            0x0090_CF54 => fake_allocate_array as *const (),
            0x009A_B992 => fake_bounds_base as *const (),
            0x0097_BF85 => fake_node_member as *const (),
            0x0099_BF1F => fake_set_fill as *const (),
            0x009A_84A0 => fake_erase_branch as *const (),
            0x009A_8947 => fake_clear_tree as *const (),
            0x0096_EDBC => fake_list_destructor as *const (),
            0x0056_50A0 => fake_string_destructor as *const (),
            0x0098_1543 => fake_vector_range as *const (),
            0x0097_C1EA => fake_next_bucket as *const (),
            0x0097_C049 => fake_hash_lookup as *const (),
            0x0096_F614 => fake_hash_reserve as *const (),
            0x0096_F209 => fake_hash_insert as *const (),
            0x0097_3C0E => fake_pointer_grow as *const (),
            _ => panic!("missing fake {address:08x}"),
        };
        crate::testing::fake(address, f as usize)
    }
    fn targets() -> Vec<u32> {
        STATE.with(|s| core::mem::take(&mut s.borrow_mut().direct_targets))
    }
    fn trace(this: u32, eax: u32, edx: u32, arguments: &[u32]) -> VirtualCall {
        VirtualCall {
            this,
            eax,
            edx,
            arguments: arguments.to_vec(),
        }
    }

    #[test]
    fn deleting_destructors_forward_members_and_only_test_the_low_flag_bit() {
        for (entry, target, adjustment, nullable, pop_ecx) in [
            (fc4p_00908902 as *const () as usize, 0x0090_86B7, 0, false, true),
            (fc4p_0090ddd5 as *const () as usize, 0x0090_DC2F, 0, false, true),
            (fc4p_0094065e as *const () as usize, 0x0094_002A, 0, false, true),
            (fc4p_00991e90 as *const () as usize, 0x0090_D964, 4, true, false),
            (fc4p_009be9af as *const () as usize, 0x0041_4250, 0x14, false, true),
        ] {
            let _destroy = install_game(target);
            let _free = install_game(0x005E_5620);
            for object in [0u32, 1, 0x1234_5678, u32::MAX] {
                for flags in [0u32, 1, 2, 3, 0x100, 0xFFFF_FFFE, u32::MAX] {
                    reset();
                    let member = if nullable && object == 0 {
                        0
                    } else {
                        object.wrapping_add(adjustment)
                    };
                    assert_eq!(
                        run(entry, object, &[flags]),
                        [
                            object,
                            if flags & 1 != 0 && pop_ecx { object } else { RESULT_ECX },
                            RESULT_EDX
                        ]
                    );
                    let mut expected = vec![trace(member, ENTRY_EAX, ENTRY_EDX, &[])];
                    let mut addresses = vec![target];
                    if flags & 1 != 0 {
                        expected.push(trace(RESULT_ECX, RESULT_EAX, RESULT_EDX, &[object]));
                        addresses.push(0x005E_5620);
                    }
                    assert_eq!(calls(), expected);
                    assert_eq!(targets(), addresses);
                }
            }
        }
        let _free = install_game(0x005E_5620);
        for flags in [0u32, 1, 2, 3, 0x100, u32::MAX] {
            reset();
            let mut object = [0xAAAA_AAAAu32, 0xBBBB_BBBB];
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 8)));
            assert_eq!(
                run(fc4p_00940bc5 as *const () as usize, this, &[flags]),
                [this, this, if flags & 1 != 0 { RESULT_EDX } else { ENTRY_EDX }]
            );
            assert_eq!(object, [0x00AD_251C, 0xBBBB_BBBB]);
            if flags & 1 != 0 {
                assert_eq!(calls(), [trace(this, ENTRY_EAX, ENTRY_EDX, &[this])]);
                assert_eq!(targets(), [0x005E_5620]);
                STATE.with(|s| assert_eq!(&s.borrow().snapshots[0][..4], &0x00AD_251Cu32.to_le_bytes()));
            } else {
                assert!(calls().is_empty());
                assert!(targets().is_empty());
            }
        }
    }

    #[test]
    fn element_allocators_skip_zero_and_pass_wrapping_byte_counts() {
        let _allocator = install_game(0x0090_CF54);
        for (entry, size, pop) in [
            (fc4p_009b3537 as *const () as usize, 40u32, 2),
            (fc4p_009db998 as *const () as usize, 4u32, 1),
        ] {
            for count in [0u32, 1, 2, 0x0666_6666, 0x4000_0000, 0x8000_0000, u32::MAX] {
                reset();
                let args = [count, 0xABCD_1234];
                let bytes = count.wrapping_mul(size);
                assert_eq!(
                    run(entry, 0x1234_5678, &args[..pop]),
                    [
                        if count == 0 { 0 } else { RESULT_EAX },
                        if count == 0 { 0x1234_5678 } else { bytes },
                        if count == 0 { ENTRY_EDX } else { RESULT_EDX }
                    ]
                );
                if count == 0 {
                    assert!(calls().is_empty());
                    assert!(targets().is_empty());
                } else {
                    assert_eq!(calls(), [trace(0x1234_5678, bytes, ENTRY_EDX, &[bytes])]);
                    assert_eq!(targets(), [0x0090_CF54]);
                }
            }
        }
    }

    #[test]
    fn bounds_and_tagged_constructors_write_only_the_original_fields() {
        let _base = install_game(0x009A_B992);
        let _member = install_game(0x0097_BF85);
        reset();
        let mut object = [0xA5u8; 64];
        let this = object.as_mut_ptr() as u32;
        let mut expected = object;
        for (offset, value) in [
            (0xC, 0u32),
            (0x10, 0x8000_0001),
            (0x14, 0x7FFF_FFFF),
            (0x18, 0x8000_0001),
            (0x1C, 0x7FFF_FFFF),
        ] {
            expected[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
        }
        expected[0x20] = 0;
        assert_eq!(
            run(fc4p_009abb63 as *const () as usize, this, &[]),
            [this, 0x8000_0001, RESULT_EDX]
        );
        assert_eq!(object, expected);
        assert_eq!(calls(), [trace(this, ENTRY_EAX, ENTRY_EDX, &[])]);
        assert_eq!(targets(), [0x009A_B992]);
        for value in [0u32, 1, 0x8000_0000, u32::MAX] {
            reset();
            let mut object = [0xA5u8; 32];
            let mut expected = object;
            let this = object.as_mut_ptr() as u32;
            expected[..4].copy_from_slice(&value.to_le_bytes());
            STATE.with(|s| s.borrow_mut().watch = Some((this, 32)));
            assert_eq!(
                run(fc4p_009db1a1 as *const () as usize, this, &[value, 0x1234_5678]),
                [this, RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(object, expected);
            assert_eq!(calls(), [trace(this + 4, value, ENTRY_EDX, &[0x1234_5678])]);
            assert_eq!(targets(), [0x0097_BF85]);
            STATE.with(|s| assert_eq!(s.borrow().snapshots, [expected.to_vec()]));
        }
    }

    #[test]
    fn fill_setter_checks_flags_after_the_callback() {
        let _setter = install_game(0x0099_BF1F);
        for flag in [0u8, 1, 0x20, 0xFF] {
            for mutate in [false, true] {
                reset();
                let mut vt = [0usize; 93];
                vt[0x170 / 4] = fake_zero as *const () as usize;
                let mut object = [0xA5u8; 0x130];
                object[..4].copy_from_slice(&(vt.as_ptr() as u32).to_le_bytes());
                object[0x128] = flag;
                let this = object.as_mut_ptr() as u32;
                let mut expected = object;
                if mutate {
                    expected[0x128..0x12C].copy_from_slice(&0x1234_5620u32.to_le_bytes());
                    STATE.with(|s| s.borrow_mut().mutation = Some((this, 0x128, 0x1234_5620)));
                }
                assert_eq!(
                    run(fc4p_009bc8b8 as *const () as usize, this, &[0xFEDC_BA98]),
                    [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                );
                let mut expected_calls = vec![trace(this, ENTRY_EAX, ENTRY_EDX, &[0xFEDC_BA98])];
                if mutate || flag & 0x20 != 0 {
                    expected_calls.push(trace(this, vt.as_ptr() as u32, RESULT_EDX, &[]));
                }
                assert_eq!(calls(), expected_calls);
                assert_eq!(targets(), [0x0099_BF1F]);
                assert_eq!(object, expected);
            }
        }
    }

    #[test]
    fn tree_branch_deletion_calls_right_subtrees_before_each_left_chain_node() {
        let _recursive = install_game(0x009A_84A0);
        let _free = install_game(0x0090_CF63);
        for n in 0..=4usize {
            reset();
            let mut nodes = [[0u32; 4]; 4];
            let ptrs: [u32; 4] = core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32);
            for i in 0..n {
                nodes[i][2] = if i + 1 < n { ptrs[i + 1] } else { 0 };
                nodes[i][3] = 0x1000 + i as u32;
            }
            let before = nodes;
            let mut expected = vec![];
            let mut addresses = vec![];
            let mut eax = ENTRY_EAX;
            let mut edx = ENTRY_EDX;
            for (i, &node) in ptrs.iter().take(n).enumerate() {
                expected.push(trace(0x1234_5678, eax, edx, &[0x1000 + i as u32]));
                addresses.push(0x009A_84A0);
                expected.push(trace(RESULT_ECX, RESULT_EAX, RESULT_EDX, &[node]));
                addresses.push(0x0090_CF63);
                eax = RESULT_EAX;
                edx = RESULT_EDX;
            }
            assert_eq!(
                run(
                    fc4p_009a84a0 as *const () as usize,
                    0x1234_5678,
                    &[if n == 0 { 0 } else { ptrs[0] }]
                ),
                [eax, if n == 0 { 0x1234_5678 } else { ptrs[n - 1] }, edx]
            );
            assert_eq!(calls(), expected);
            assert_eq!(targets(), addresses);
            assert_eq!(nodes, before);
        }
    }

    #[test]
    fn tree_clear_reloads_replaced_header_and_only_resets_nonempty_trees() {
        let _erase = install_game(0x009A_84A0);
        for count in [0u32, 1, 0x8000_0000, u32::MAX] {
            for replace in [false, true] {
                reset();
                let mut header = [0xAAAA_AAAAu32; 5];
                header[1] = 0x1234_5678;
                let p = header.as_mut_ptr() as u32;
                let mut other = [0xBBBB_BBBBu32; 5];
                let q = other.as_mut_ptr() as u32;
                let mut object = [p, count, 0xCCCC_CCCC];
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| {
                    let mut s = s.borrow_mut();
                    s.watch = Some((p, 20));
                    if replace {
                        s.mutation = Some((this, 0, q));
                    }
                });
                let got = run(fc4p_009a8947 as *const () as usize, this, &[]);
                if count == 0 {
                    assert_eq!(got, [ENTRY_EAX, this, ENTRY_EDX]);
                    assert!(calls().is_empty());
                    assert!(targets().is_empty());
                } else {
                    let chosen = if replace { q } else { p };
                    assert_eq!(got, [chosen, RESULT_ECX, RESULT_EDX]);
                    assert_eq!(object, [chosen, 0, 0xCCCC_CCCC]);
                    assert_eq!(
                        if replace { other } else { header },
                        [
                            if replace { 0xBBBB_BBBB } else { 0xAAAA_AAAA },
                            0,
                            chosen,
                            chosen,
                            if replace { 0xBBBB_BBBB } else { 0xAAAA_AAAA }
                        ]
                    );
                    assert_eq!(calls(), [trace(this, p, ENTRY_EDX, &[0x1234_5678])]);
                    assert_eq!(targets(), [0x009A_84A0]);
                    STATE.with(|s| assert_eq!(&s.borrow().snapshots[0][4..8], &0x1234_5678u32.to_le_bytes()));
                }
            }
        }
    }

    #[test]
    fn tree_destructor_rereads_header_and_frees_only_nonnull_storage() {
        let _clear = install_game(0x009A_8947);
        let _free = install_game(0x0090_CF63);
        for original in [0u32, 0x1234_5678] {
            for replacement in [0u32, 0x8765_4321] {
                reset();
                let mut object = [original, 0xABCD_1234];
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| s.borrow_mut().mutation = Some((this, 0, replacement)));
                assert_eq!(
                    run(fc4p_009a9427 as *const () as usize, this, &[]),
                    [
                        RESULT_EAX,
                        if replacement == 0 { RESULT_ECX } else { replacement },
                        RESULT_EDX
                    ]
                );
                let mut expected = vec![trace(this, ENTRY_EAX, ENTRY_EDX, &[])];
                let mut addresses = vec![0x009A_8947];
                if replacement != 0 {
                    expected.push(trace(RESULT_ECX, RESULT_EAX, RESULT_EDX, &[replacement]));
                    addresses.push(0x0090_CF63);
                }
                assert_eq!(calls(), expected);
                assert_eq!(targets(), addresses);
                assert_eq!(object, [replacement, 0xABCD_1234]);
            }
        }
    }

    #[test]
    fn list_clear_releases_nodes_in_forward_order_and_restores_header_links() {
        let _free = install_game(0x0090_CF63);
        for n in 0..=4usize {
            reset();
            let mut header = [0u32; 3];
            let p = header.as_mut_ptr() as u32;
            let mut nodes = [[0u32; 3]; 4];
            let ptrs: [u32; 4] = core::array::from_fn(|i| nodes[i].as_mut_ptr() as u32);
            header = [
                if n == 0 { p } else { ptrs[0] },
                if n == 0 { p } else { ptrs[n - 1] },
                0xABCD_1234,
            ];
            for i in 0..n {
                nodes[i] = [if i + 1 == n { p } else { ptrs[i + 1] }, 0x1111_1111, 0x2222_2222];
            }
            let before = nodes;
            let mut object = [p, 0xAAAA_AAAA];
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_009a841f as *const () as usize, this, &[]),
                [
                    p,
                    if n == 0 { this } else { ptrs[n - 1] },
                    if n == 0 { ENTRY_EDX } else { RESULT_EDX }
                ]
            );
            let expected: Vec<_> = (0..n)
                .map(|i| {
                    trace(
                        if i == 0 { this } else { ptrs[i - 1] },
                        ptrs[i],
                        if i == 0 { ENTRY_EDX } else { RESULT_EDX },
                        &[ptrs[i]],
                    )
                })
                .collect();
            assert_eq!(calls(), expected);
            assert_eq!(targets(), vec![0x0090_CF63; n]);
            assert_eq!(header, [p, p, 0xABCD_1234]);
            assert_eq!(nodes, before);
            assert_eq!(object, [p, 0xAAAA_AAAA]);
        }
    }

    #[test]
    fn member_destructors_forward_in_order_and_wrap_the_global_counter() {
        let _list = install_game(0x0096_EDBC);
        let _text = install_game(0x0041_4250);
        let _string = install_game(0x0056_50A0);
        reset();
        let mut object = [0xA5u8; 128];
        let before = object;
        let this = object.as_mut_ptr() as u32;
        assert_eq!(
            run(fc4p_00945f24 as *const () as usize, this, &[]),
            [RESULT_EAX, RESULT_ECX, RESULT_EDX]
        );
        assert_eq!(
            calls(),
            [
                trace(this + 0x64, ENTRY_EAX, ENTRY_EDX, &[]),
                trace(this + 0xC, RESULT_EAX, RESULT_EDX, &[])
            ]
        );
        assert_eq!(targets(), [0x0096_EDBC, 0x0041_4250]);
        assert_eq!(object, before);
        for mut counter in [0u32, 1, 0x8000_0000, u32::MAX] {
            reset();
            let start = counter;
            let _counter = crate::testing::fake_data(0x00B6_33B8, (&mut counter as *mut u32).cast());
            let mut expected = object;
            expected[..4].copy_from_slice(&0x00AD_4648u32.to_le_bytes());
            STATE.with(|s| s.borrow_mut().watch = Some((this, 128)));
            assert_eq!(
                run(fc4p_00943a0e as *const () as usize, this, &[]),
                [RESULT_EAX, RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(counter, start.wrapping_sub(1));
            assert_eq!(object, expected);
            assert_eq!(
                calls(),
                [
                    trace(this + 0xC, ENTRY_EAX, ENTRY_EDX, &[]),
                    trace(this + 8, RESULT_EAX, RESULT_EDX, &[])
                ]
            );
            assert_eq!(targets(), [0x0056_50A0, 0x0056_50A0]);
            STATE.with(|s| assert_eq!(s.borrow().snapshots, [expected.to_vec(), expected.to_vec()]));
        }
    }

    #[test]
    fn vector_destruction_reloads_storage_and_keeps_cdecl_pop_results() {
        let _range = install_game(0x0098_1543);
        let _free = install_game(0x0090_CF63);
        for original in [0u32, 0x1234_5678] {
            for replacement in [0u32, 0x8765_4321] {
                reset();
                let mut object = [original, 0x1122_3344, 0xAAAA_AAAA];
                let this = object.as_mut_ptr() as u32;
                STATE.with(|s| s.borrow_mut().mutation = Some((this, 0, replacement)));
                assert_eq!(
                    run(fc4p_0093de49 as *const () as usize, this, &[]),
                    [
                        RESULT_EAX,
                        if replacement == 0 { 0x1122_3344 } else { replacement },
                        RESULT_EDX
                    ]
                );
                let mut expected = vec![trace(this, ENTRY_EAX, ENTRY_EDX, &[original, 0x1122_3344])];
                let mut addresses = vec![0x0098_1543];
                if replacement != 0 {
                    expected.push(trace(0x1122_3344, RESULT_EAX, RESULT_EDX, &[replacement]));
                    addresses.push(0x0090_CF63);
                }
                assert_eq!(calls(), expected);
                assert_eq!(targets(), addresses);
                assert_eq!(object, [replacement, 0x1122_3344, 0xAAAA_AAAA]);
            }
        }
    }

    #[test]
    fn hash_iterator_advance_follows_links_or_calls_the_next_bucket_entry() {
        let _next = install_game(0x0097_C1EA);
        for successor in [0u32, 1, 0x8765_4321, u32::MAX] {
            reset();
            let mut node = [successor, 0xAAAA_AAAA];
            let mut object = [node.as_mut_ptr() as u32, 0xBBBB_BBBB];
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_0096f5b7 as *const () as usize, this, &[]),
                [
                    this,
                    if successor == 0 { RESULT_ECX } else { this },
                    if successor == 0 { RESULT_EDX } else { ENTRY_EDX }
                ]
            );
            assert_eq!(
                object,
                [if successor == 0 { RESULT_EAX } else { successor }, 0xBBBB_BBBB]
            );
            if successor == 0 {
                assert_eq!(calls(), [trace(this, 0, ENTRY_EDX, &[])]);
                assert_eq!(targets(), [0x0097_C1EA]);
            } else {
                assert!(calls().is_empty());
                assert!(targets().is_empty());
            }
        }
    }

    #[test]
    fn hash_lookup_writes_the_iterator_after_the_lookup_call() {
        let _lookup = install_game(0x0097_C049);
        for offset in [0usize, 4, 8, 16] {
            reset();
            let mut object = [0xA5u8; 32];
            let mut expected = object;
            let this = object.as_mut_ptr() as u32;
            let output = this + offset as u32;
            expected[offset..offset + 4].copy_from_slice(&RESULT_EAX.to_le_bytes());
            expected[offset + 4..offset + 8].copy_from_slice(&this.to_le_bytes());
            STATE.with(|s| s.borrow_mut().watch = Some((this, 32)));
            assert_eq!(
                run(fc4p_0096f290 as *const () as usize, this, &[output, 0x1234_5678]),
                [output, output, RESULT_EDX]
            );
            assert_eq!(object, expected);
            assert_eq!(calls(), [trace(this, ENTRY_EAX, ENTRY_EDX, &[0x1234_5678])]);
            assert_eq!(targets(), [0x0097_C049]);
            STATE.with(|s| assert_eq!(s.borrow().snapshots, [vec![0xA5; 32]]));
        }
    }

    #[test]
    fn insertion_reserves_wrapping_count_then_forwards_the_original_arguments() {
        let _reserve = install_game(0x0096_F614);
        let _insert = install_game(0x0096_F209);
        for count in [0u32, 1, 0x8000_0000, u32::MAX] {
            reset();
            let mut object = [0xAAAA_AAAAu32; 6];
            object[4] = count;
            let before = object;
            let this = object.as_mut_ptr() as u32;
            let next = count.wrapping_add(1);
            assert_eq!(
                run(fc4p_0097005c as *const () as usize, this, &[0x1234_5678, 0x8765_4321]),
                [0x1234_5678, RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(
                calls(),
                [
                    trace(this, next, ENTRY_EDX, &[next]),
                    trace(this, RESULT_EAX, RESULT_EDX, &[0x1234_5678, 0x8765_4321])
                ]
            );
            assert_eq!(targets(), [0x0096_F614, 0x0096_F209]);
            assert_eq!(object, before);
        }
    }

    #[test]
    fn capacity_check_uses_wrapping_signed_pointer_difference_and_unsigned_limits() {
        let _grow = install_game(0x0097_3C0E);
        for (begin, end, capacity) in [
            (0u32, 0u32, 0u32),
            (0x1000, 0x1040, 20),
            (0x1040, 0x1000, 20),
            (0x8000_0000, 0, 0),
            (0, 0x8000_0000, u32::MAX),
        ] {
            for requested in [0u32, 1, 3, 4, 19, 20, 0x7FFF_FFFF, u32::MAX] {
                reset();
                let mut object = [0xAAAA_AAAAu32; 12];
                object[7] = begin;
                object[8] = end;
                object[9] = capacity;
                let before = object;
                let this = object.as_mut_ptr() as u32;
                let occupied = ((begin.wrapping_sub(end) as i32) >> 2) as u32;
                let remaining = capacity.wrapping_sub(occupied);
                let grow = requested.wrapping_add(1) > remaining;
                assert_eq!(
                    run(fc4p_00982d95 as *const () as usize, this, &[requested]),
                    if grow {
                        [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                    } else {
                        [requested, this, remaining]
                    }
                );
                if grow {
                    assert_eq!(calls(), [trace(this, requested, remaining, &[requested, 0])]);
                    assert_eq!(targets(), [0x0097_3C0E]);
                } else {
                    assert!(calls().is_empty());
                    assert!(targets().is_empty());
                }
                assert_eq!(object, before);
            }
        }
    }

    #[test]
    fn locked_release_brackets_count_changes_and_calls_cleanup_for_zero_or_one() {
        for count in [0u32, 1, 2, 0x8000_0000, u32::MAX] {
            reset();
            let mut vt = [0usize; 32];
            vt[0x7C / 4] = fake_zero as *const () as usize;
            let mut lock_vt = [0usize; 3];
            lock_vt[1] = fake_zero as *const () as usize;
            lock_vt[2] = fake_zero as *const () as usize;
            let mut object = [vt.as_ptr() as u32, count, lock_vt.as_ptr() as u32, 0xAAAA_AAAA];
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 16)));
            let remaining = count.saturating_sub(1);
            assert_eq!(
                run(fc4p_009756f6 as *const () as usize, this, &[]),
                [remaining, RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(object[1], if count > 1 { remaining } else { count });
            let mut expected = vec![
                trace(this + 8, lock_vt.as_ptr() as u32, ENTRY_EDX, &[]),
                trace(this + 8, lock_vt.as_ptr() as u32, RESULT_EDX, &[]),
            ];
            if count <= 1 {
                expected.push(trace(this, vt.as_ptr() as u32, RESULT_EDX, &[]));
            }
            assert_eq!(calls(), expected);
            STATE.with(|s| {
                let s = s.borrow();
                assert_eq!(&s.snapshots[0][4..8], &count.to_le_bytes());
                assert_eq!(&s.snapshots[1][4..8], &object[1].to_le_bytes());
            });
        }
    }

    #[test]
    fn work_buffer_shutdown_clears_active_before_stopping_and_frees_even_null() {
        let _free = install_game(0x005E_5620);
        for active in [0u8, 1, 0xFF] {
            for mode in [0u8, 1, 2, 3, 4, 0xFF] {
                for buffer in [0u32, 0x1234_5678] {
                    reset();
                    let mut vt = [0usize; 9];
                    vt[8] = fake_zero as *const () as usize;
                    let mut object = [0xA5u8; 0x1D0];
                    object[4..8].copy_from_slice(&(vt.as_ptr() as u32).to_le_bytes());
                    object[0x14] = active;
                    object[0x1C] = mode;
                    object[0x1C4..0x1C8].copy_from_slice(&buffer.to_le_bytes());
                    let this = object.as_mut_ptr() as u32;
                    let mut expected = object;
                    let stop = mode & 3 != 0;
                    STATE.with(|s| s.borrow_mut().watch = Some((this, 0x1D0)));
                    if active != 0 {
                        expected[0x14] = 0;
                        expected[0x1C4..0x1C8].fill(0);
                    }
                    assert_eq!(
                        run(fc4p_0096df8d as *const () as usize, this, &[]),
                        [
                            if active == 0 {
                                (ENTRY_EAX & !255) | 1
                            } else {
                                (RESULT_EAX & !255) | 1
                            },
                            if active == 0 { this } else { buffer },
                            if active == 0 { ENTRY_EDX } else { RESULT_EDX }
                        ]
                    );
                    let mut expected_calls = vec![];
                    if active != 0 {
                        if stop {
                            expected_calls.push(trace(this + 4, vt.as_ptr() as u32, ENTRY_EDX, &[]));
                        }
                        expected_calls.push(trace(
                            if stop { RESULT_ECX } else { this },
                            if stop { RESULT_EAX } else { ENTRY_EAX },
                            if stop { RESULT_EDX } else { ENTRY_EDX },
                            &[buffer],
                        ));
                        assert_eq!(targets(), [0x005E_5620]);
                        STATE.with(|s| {
                            for snapshot in &s.borrow().snapshots {
                                assert_eq!(snapshot[0x14], 0);
                                assert_eq!(&snapshot[0x1C4..0x1C8], &buffer.to_le_bytes());
                            }
                        });
                    } else {
                        assert!(targets().is_empty());
                    }
                    assert_eq!(calls(), expected_calls);
                    assert_eq!(object, expected);
                }
            }
        }
    }

    #[test]
    fn utf8_counts_cover_every_byte_and_ignore_upper_argument_bits() {
        for upper in [0u32, 0x1234_5600, 0xFFFF_FF00] {
            for byte in 0..=255u32 {
                let want = if byte < 0xC2 {
                    0
                } else if byte <= 0xDF {
                    1
                } else if byte <= 0xEF {
                    2
                } else if byte <= 0xF7 {
                    3
                } else if byte <= 0xFB {
                    4
                } else if byte <= 0xFD {
                    5
                } else {
                    0
                };
                assert_eq!(
                    invoke(
                        fc4p_00910128 as *const () as usize,
                        0x1234,
                        &[upper | byte],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true
                    ),
                    [want, 0x1234, ENTRY_EDX]
                );
            }
        }
    }
    #[test]
    fn codepage_predicate_checks_all_bytes_and_retains_unknown_encoding_upper_bytes() {
        for encoding in [
            0u32,
            1,
            100,
            931,
            932,
            933,
            935,
            936,
            937,
            948,
            949,
            950,
            951,
            1200,
            0x8000_0000,
            u32::MAX,
        ] {
            for upper in [0u32, 0xFFFF_FF00] {
                for byte in 0..=255u32 {
                    let want = match encoding {
                        932 => u32::from((129..=159).contains(&byte) || (224..=252).contains(&byte)),
                        936 => u32::from((161..=252).contains(&byte)),
                        949 => u32::from((176..=200).contains(&byte)),
                        950 => u32::from((129..=254).contains(&byte)),
                        _ => encoding.wrapping_sub(950) & 0xFFFF_FF00,
                    };
                    assert_eq!(
                        invoke(
                            fc4p_0091030f as *const () as usize,
                            0x1234,
                            &[upper | byte, encoding],
                            ENTRY_EAX,
                            ENTRY_EDX,
                            true
                        ),
                        [want, 0x1234, ENTRY_EDX]
                    );
                }
            }
        }
    }

    thread_local! { static ENCODING_RESULT: std::cell::Cell<u32> = const { std::cell::Cell::new(0) }; }
    unsafe extern "stdcall" fn observe_encoding(frame: *mut Registers, address: u32, count: usize) {
        // SAFETY: the fake's frame contains its original registers; the chosen result is local to this test.
        unsafe {
            observe_direct(frame, address, count);
            (*frame).eax = ENCODING_RESULT.with(|v| v.get());
            (*frame).ecx = RESULT_ECX;
            (*frame).edx = RESULT_EDX;
        }
    }
    macro_rules! encoding_fake {
        ($name:ident,$address:literal,$count:literal) => {
            #[unsafe(naked)]
            unsafe extern "C" fn $name() {
                core::arch::naked_asm!(
                    "push edx","push ecx","push eax","mov eax, esp","push {count}","push {address}","push eax","call {observe}",
                    "pop eax","pop ecx","pop edx","ret",
                    count=const $count,address=const $address,observe=sym observe_encoding,
                );
            }
        };
    }
    encoding_fake!(fake_current_encoding, 0x0091_0014, 0);
    encoding_fake!(fake_encoding_predicate, 0x0091_030F, 2);
    encoding_fake!(fake_utf8_predicate, 0x0091_03B0, 1);
    encoding_fake!(fake_utf8_count, 0x0091_0128, 1);
    encoding_fake!(fake_encoding_count, 0x0091_036D, 2);

    #[test]
    fn codepage_count_normalizes_only_the_predicate_low_byte() {
        let _predicate = crate::testing::fake(0x0091_030F, fake_encoding_predicate as *const () as usize);
        for result in [0u32, 1, 0xFF, 0x100, 0x8000_0000, u32::MAX] {
            reset();
            ENCODING_RESULT.with(|v| v.set(result));
            assert_eq!(
                invoke(
                    fc4p_0091036d as *const () as usize,
                    0x1234,
                    &[0xABCD_12EF, 0x8765_4321],
                    ENTRY_EAX,
                    ENTRY_EDX,
                    true
                ),
                [u32::from(result & 255 != 0), RESULT_ECX, RESULT_EDX]
            );
            assert_eq!(
                calls(),
                [trace(0x1234, ENTRY_EAX, ENTRY_EDX, &[0xABCD_12EF, 0x8765_4321])]
            );
            assert_eq!(targets(), [0x0091_030F]);
        }
    }
    #[test]
    fn encoding_adapters_cover_sentinels_signed_thresholds_and_cleanup_registers() {
        let _current = crate::testing::fake(0x0091_0014, fake_current_encoding as *const () as usize);
        let _predicate = crate::testing::fake(0x0091_030F, fake_encoding_predicate as *const () as usize);
        let _utf8_predicate = crate::testing::fake(0x0091_03B0, fake_utf8_predicate as *const () as usize);
        let _utf8_count = crate::testing::fake(0x0091_0128, fake_utf8_count as *const () as usize);
        let _count = crate::testing::fake(0x0091_036D, fake_encoding_count as *const () as usize);
        for count in [false, true] {
            let entry = if count {
                fc4p_009108ed as *const () as usize
            } else {
                fc4p_009108b4 as *const () as usize
            };
            for encoding in [
                0u32,
                8,
                99,
                100,
                932,
                0x7FFF_FFFF,
                0x8000_0000,
                u32::MAX - 2,
                u32::MAX - 1,
                u32::MAX,
            ] {
                for result in [0u32, 1, 8, 99, 100, 932, 0x8000_0000, u32::MAX] {
                    reset();
                    ENCODING_RESULT.with(|v| v.set(result));
                    let effective = if encoding == u32::MAX { result } else { encoding };
                    let utf8 = encoding == u32::MAX - 1 || effective == 8;
                    let delegated = utf8 || (effective as i32) >= 100;
                    let eax = if delegated {
                        result
                    } else if count {
                        0
                    } else {
                        effective & !255
                    };
                    let ecx = if utf8 && !count {
                        0xABCD_12EF
                    } else if delegated || encoding == u32::MAX {
                        RESULT_ECX
                    } else {
                        0x1234
                    };
                    let edx = if delegated || encoding == u32::MAX {
                        RESULT_EDX
                    } else {
                        ENTRY_EDX
                    };
                    assert_eq!(
                        invoke(entry, 0x1234, &[0xABCD_12EF, encoding], ENTRY_EAX, ENTRY_EDX, true),
                        [eax, ecx, edx]
                    );
                    let mut expected = vec![];
                    let mut addresses = vec![];
                    if encoding == u32::MAX {
                        expected.push(trace(0x1234, u32::MAX, ENTRY_EDX, &[]));
                        addresses.push(0x0091_0014);
                    }
                    if delegated {
                        let args = if utf8 {
                            vec![0xABCD_12EF]
                        } else {
                            vec![0xABCD_12EF, effective]
                        };
                        expected.push(trace(
                            if encoding == u32::MAX { RESULT_ECX } else { 0x1234 },
                            effective,
                            if encoding == u32::MAX { RESULT_EDX } else { ENTRY_EDX },
                            &args,
                        ));
                        addresses.push(if utf8 {
                            if count {
                                0x0091_0128
                            } else {
                                0x0091_03B0
                            }
                        } else if count {
                            0x0091_036D
                        } else {
                            0x0091_030F
                        });
                    }
                    assert_eq!(calls(), expected);
                    assert_eq!(targets(), addresses);
                }
            }
        }
    }
    #[test]
    fn hash_begin_checks_every_first_bucket_position_and_overlapping_outputs() {
        for n in 0..=7usize {
            for first in 0..=n {
                for offset in [0usize, 4, 12, 20] {
                    let mut buckets = [0u32; 7];
                    if first < n {
                        buckets[first] = 0x8765_4321;
                    }
                    let begin = buckets.as_mut_ptr() as u32;
                    let mut object = [0xAAAA_AAAAu32; 9];
                    object[1] = begin;
                    object[2] = begin + (n as u32) * 4;
                    let this = object.as_mut_ptr() as u32;
                    let output = this + offset as u32;
                    let mut expected = object;
                    expected[offset / 4] = if first < n { 0x8765_4321 } else { 0 };
                    expected[offset / 4 + 1] = this;
                    let edx = if first < n { 0x8765_4321 } else { n as u32 };
                    let before = buckets;
                    assert_eq!(
                        run(fc4p_0097c081 as *const () as usize, this, &[output]),
                        [output, this, edx]
                    );
                    assert_eq!(object, expected);
                    assert_eq!(buckets, before);
                }
            }
        }
    }

    mod resource_ports {
        use super::*;

        #[derive(Default)]
        struct Batch {
            hook: Option<unsafe fn(&mut Registers, u32)>,
            values: [u32; 8],
        }

        thread_local! { static BATCH: RefCell<Batch> = RefCell::new(Batch::default()); }

        fn start(hook: Option<unsafe fn(&mut Registers, u32)>, values: [u32; 8]) {
            reset();
            BATCH.with(|s| *s.borrow_mut() = Batch { hook, values });
        }

        unsafe extern "stdcall" fn dispatch(frame: *mut Registers, address: u32, count: usize) {
            // SAFETY: each fake supplies its entry frame and exact stack argument count.
            unsafe {
                observe_direct(frame, address, count);
                let hook = BATCH.with(|s| s.borrow().hook);
                if let Some(hook) = hook {
                    hook(&mut *frame, address);
                } else {
                    (*frame).eax = RESULT_EAX;
                    (*frame).ecx = RESULT_ECX;
                    (*frame).edx = RESULT_EDX;
                }
            }
        }

        fn results(r: &mut Registers) {
            r.eax = RESULT_EAX;
            r.ecx = RESULT_ECX;
            r.edx = RESULT_EDX;
        }

        macro_rules! batch_fake {
            ($name:ident, $address:literal, $count:literal, $pop:literal) => {
                #[unsafe(naked)]
                unsafe extern "C" fn $name() {
                    core::arch::naked_asm!(
                        "push edx", "push ecx", "push eax", "mov eax, esp",
                        "push {count}", "push {address}", "push eax", "call {dispatch}",
                        "pop eax", "pop ecx", "pop edx", "ret {pop}",
                        count = const $count, address = const $address as u32, pop = const $pop,
                        dispatch = sym dispatch,
                    );
                }
            };
        }

        batch_fake!(insert_list, 0x0089_7C8D, 3, 12);
        batch_fake!(grow_vector, 0x0099_4D77, 5, 20);
        batch_fake!(clear_range, 0x009A_1ADD, 2, 8);
        batch_fake!(advance_node, 0x0040_D4C0, 1, 0);
        batch_fake!(clear_resources, 0x008F_FD4F, 0, 0);
        batch_fake!(copy_element, 0x009A_7610, 2, 0);
        batch_fake!(lock, 0x0093_0B35, 0, 0);
        batch_fake!(unlock, 0x0093_0B6D, 0, 0);
        batch_fake!(small_allocate, 0x0090_D6AB, 1, 4);
        batch_fake!(large_allocate, 0x0090_D494, 1, 4);
        batch_fake!(small_free, 0x0090_D7F2, 2, 8);
        batch_fake!(large_free, 0x0090_D5DB, 2, 8);
        batch_fake!(construct_tree, 0x0093_F7EB, 1, 4);
        batch_fake!(retain_value, 0x004F_20C0, 1, 4);
        batch_fake!(insert_value, 0x0096_90F4, 1, 4);
        batch_fake!(search_mapping, 0x0079_FA90, 5, 0);
        batch_fake!(bad_position, 0x0040_7C40, 0, 0);
        batch_fake!(bad_length, 0x0079_C6E0, 0, 0);
        batch_fake!(insert_bytes, 0x005C_2910, 3, 12);
        batch_fake!(construct_guard, 0x0048_C9F0, 1, 4);
        batch_fake!(release_value, 0x0050_5190, 1, 4);
        batch_fake!(message_key, 0x0044_A490, 1, 4);
        batch_fake!(message_allocate, 0x005E_55E0, 1, 0);
        batch_fake!(message_construct, 0x0097_80DC, 2, 8);
        batch_fake!(query_message, 0xFFFF_0200, 2, 8);
        batch_fake!(free_storage, 0x0090_CF63, 1, 0);
        batch_fake!(delete_value, 0xFFFF_0000, 1, 4);
        batch_fake!(release_interface, 0xFFFF_0008, 0, 0);
        batch_fake!(delete_window, 0xFFFF_0070, 1, 4);
        batch_fake!(release_resource, 0xFFFF_0058, 1, 4);
        batch_fake!(allocate_node, 0xFFFF_0100, 0, 0);

        fn install(address: u32) -> crate::testing::Fake {
            let entry = match address {
                0x0089_7C8D => insert_list as *const (),
                0x0099_4D77 => grow_vector as *const (),
                0x009A_1ADD => clear_range as *const (),
                0x0040_D4C0 => advance_node as *const (),
                0x008F_FD4F => clear_resources as *const (),
                0x009A_7610 => copy_element as *const (),
                0x0093_0B35 => lock as *const (),
                0x0093_0B6D => unlock as *const (),
                0x0090_D6AB => small_allocate as *const (),
                0x0090_D494 => large_allocate as *const (),
                0x0090_D7F2 => small_free as *const (),
                0x0090_D5DB => large_free as *const (),
                0x0093_F7EB => construct_tree as *const (),
                0x004F_20C0 => retain_value as *const (),
                0x0096_90F4 => insert_value as *const (),
                0x0079_FA90 => search_mapping as *const (),
                0x0040_7C40 => bad_position as *const (),
                0x0079_C6E0 => bad_length as *const (),
                0x005C_2910 => insert_bytes as *const (),
                0x0048_C9F0 => construct_guard as *const (),
                0x0050_5190 => release_value as *const (),
                0x0044_A490 => message_key as *const (),
                0x005E_55E0 => message_allocate as *const (),
                0x0097_80DC => message_construct as *const (),
                0x0090_CF63 => free_storage as *const (),
                _ => panic!("missing fake {address:08x}"),
            };
            crate::testing::fake(address, entry as usize)
        }

        fn install_many(addresses: &[u32]) -> Vec<crate::testing::Fake> {
            addresses.iter().map(|&a| install(a)).collect()
        }

        mod added_helpers {
            use super::*;

            batch_fake!(copy_records, 0x0090_91B2, 5, 0);
            batch_fake!(move_records, 0x0093_352F, 4, 0);
            batch_fake!(retained_copy, 0x0094_0627, 1, 4);
            batch_fake!(reset_value, 0x0093_AB29, 1, 4);
            batch_fake!(copy_pair_value, 0x0099_7792, 2, 0);
            batch_fake!(copy_hash, 0x0096_E78B, 1, 4);
            batch_fake!(tree_tags, 0x0093_7FAD, 2, 8);
            batch_fake!(insert_pair, 0x0096_F12D, 2, 8);
            batch_fake!(initialize_vector, 0x009D_16D9, 2, 8);
            batch_fake!(fill_vector, 0x0041_86F0, 3, 0);
            batch_fake!(compare_value, 0x0043_1180, 1, 4);
            batch_fake!(erase_iterator, 0x0097_C14C, 1, 4);
            batch_fake!(destroy_elements, 0x0093_B776, 3, 0);
            batch_fake!(find_key, 0x0094_C8F5, 1, 4);
            batch_fake!(destroy_title, 0x0099_D2FE, 0, 0);
            batch_fake!(destroy_base, 0x0099_E1A2, 0, 0);
            batch_fake!(construct_base, 0x0099_D938, 0, 0);
            batch_fake!(erase_first, 0x0094_9259, 2, 8);
            batch_fake!(tree_hint, 0x0098_1404, 3, 12);
            batch_fake!(allocate_storage, 0x0090_CF54, 1, 0);
            batch_fake!(erase_record, 0x0093_354C, 1, 4);
            batch_fake!(lookup_three_words, 0x0096_EEA9, 1, 4);
            batch_fake!(remember_pair, 0x0096_F594, 2, 8);
            batch_fake!(seek_stream, 0xFFFF_0030, 1, 4);
            batch_fake!(resize_stream, 0xFFFF_0034, 1, 4);
            batch_fake!(read_stream, 0xFFFF_0038, 2, 8);
            batch_fake!(write_stream, 0xFFFF_0040, 2, 8);
            batch_fake!(init_directory, 0xFFFF_00A0, 3, 12);
            batch_fake!(refresh_style, 0x009C_1E6D, 0, 0);
            batch_fake!(refresh_title, 0xFFFF_0170, 0, 0);

            unsafe extern "thiscall" fn record_entry_stack(frame: *mut Registers) {
                BATCH.with(|s| s.borrow_mut().values[2] = frame as u32 + 12);
            }

            macro_rules! stack_checked_entry {
                ($name:ident, $target:ident) => {
                    #[unsafe(naked)]
                    unsafe extern "C" fn $name() {
                        core::arch::naked_asm!(
                            "push edx", "push ecx", "push eax", "mov ecx, esp", "call {record}",
                            "pop eax", "pop ecx", "pop edx", "jmp {target}",
                            record = sym record_entry_stack, target = sym $target,
                        );
                    }
                };
            }
            stack_checked_entry!(checked_remember_range, fc4p_0096fde2);
            stack_checked_entry!(checked_erase_first, fc4p_009d6185);

            unsafe fn guarded_hook(r: &mut Registers, address: u32) {
                let values = BATCH.with(|s| s.borrow().values);
                let mut answer = RESULT_EAX;
                // SAFETY: the tests supply live guard storage, object fields, copied keys and pair outputs.
                unsafe {
                    match address {
                        0x0048_C9F0 => {
                            put_word(r.ecx, 0, 0xDEAD_BEEF);
                            put_word(r.ecx, 4, values[0]);
                        }
                        0xFFFF_0030 => {
                            let delta = if values[4] != 0 { 7 } else { 0 };
                            if delta != 0 {
                                put_word(values[1], 0xC0, word(values[1], 0xC0).wrapping_add(delta));
                            }
                            answer = r.argument(0).wrapping_add(delta).wrapping_add(values[2]);
                        }
                        0xFFFF_0038 | 0xFFFF_0040 | 0xFFFF_00A0 => answer = values[3],
                        0x0096_EEA9 => {
                            let key = r.argument(0);
                            assert_eq!([word(key, 0), word(key, 4), word(key, 8)], [0x1234, 0x5678, 0x9ABC]);
                            answer = values[5];
                        }
                        0x0096_F594 => {
                            let pair = r.argument(1);
                            assert_eq!([word(pair, 0), word(pair, 4)], [values[6], values[7]]);
                            assert_eq!(r.argument(0), values[2] - 20);
                            assert_eq!(pair, values[2] - 12);
                            put_word(r.argument(0), 0, 0x1234_5678);
                            (r.argument(0).wrapping_add(4) as *mut u8).write(1);
                            answer = r.argument(0);
                        }
                        _ => {}
                    }
                }
                results(r);
                r.eax = answer;
            }

            fn guarded_start(values: [u32; 8], watch: Option<(u32, usize)>) {
                start(Some(guarded_hook), values);
                STATE.with(|s| s.borrow_mut().watch = watch);
            }

            #[test]
            fn guarded_options_store_full_word_before_releasing_the_lock() {
                let _guard = install(0x0048_C9F0);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32, 0xA5A5_A5A5];
                let lock_ptr = lock.as_mut_ptr() as u32;
                for options in [0, 1, 2, 0x8000_0000, u32::MAX] {
                    let mut object = [0xA5u8; 0x150];
                    let this = object.as_mut_ptr() as u32;
                    guarded_start([lock_ptr, this, 0, 0, 0, 0, 0, 0], Some((this, object.len())));
                    assert_eq!(
                        run(fc4p_0096e15f as *const () as usize, this, &[options]),
                        [(RESULT_EAX & !255) | 1, RESULT_ECX, RESULT_EDX]
                    );
                    let mut expected = [0xA5u8; 0x150];
                    expected[0x144..0x148].copy_from_slice(&options.to_le_bytes());
                    assert_eq!(object, expected);
                    let snapshots = STATE.with(|s| s.borrow().snapshots.clone());
                    assert_eq!(snapshots[0], [0xA5; 0x150]);
                    assert_eq!(snapshots[1], expected);
                    assert_eq!(targets(), [0x0048_C9F0, 0xFFFF_0008]);
                    let log = calls();
                    assert_eq!(log[0].eax, this + 12);
                    assert_eq!(log[0].arguments, [this + 12]);
                    assert_eq!(log[1], trace(lock_ptr, lock_table.as_ptr() as u32, RESULT_EDX, &[]));
                }
            }

            #[test]
            fn stream_resize_calls_the_embedded_stream_only_when_both_mode_bits_are_clear() {
                let _guard = install(0x0048_C9F0);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32];
                let lock_ptr = lock.as_mut_ptr() as u32;
                let mut table = [0usize; 14];
                table[13] = resize_stream as *const () as usize;
                for flags in [0u32, 1, 2, 3, 4, 0x100, u32::MAX] {
                    let mut object = [0xA5A5_A5A5u32; 0x60];
                    let this = object.as_mut_ptr() as u32;
                    object[0x1C / 4] = flags;
                    object[0x2C / 4] = table.as_ptr() as u32;
                    let before = object;
                    guarded_start([lock_ptr, this, 0, 0, 0, 0, 0, 0], None);
                    let allowed = flags & 3 == 0;
                    assert_eq!(
                        run(fc4p_0096dff0 as *const () as usize, this, &[0xFEDCBA98]),
                        [(RESULT_EAX & !255) | u32::from(allowed), RESULT_ECX, RESULT_EDX]
                    );
                    assert_eq!(object, before);
                    assert_eq!(
                        targets(),
                        if allowed {
                            vec![0x0048_C9F0, 0xFFFF_0034, 0xFFFF_0008]
                        } else {
                            vec![0x0048_C9F0, 0xFFFF_0008]
                        }
                    );
                    let log = calls();
                    if allowed {
                        assert_eq!(
                            log[1],
                            trace(this + 0x2C, table.as_ptr() as u32, RESULT_EDX, &[0xFEDCBA98])
                        );
                    }
                }
            }

            #[test]
            fn offset_transfers_check_permissions_seek_result_and_reloaded_base_then_release() {
                let _guard = install(0x0048_C9F0);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32];
                let lock_ptr = lock.as_mut_ptr() as u32;
                let mut table = [0usize; 17];
                table[12] = seek_stream as *const () as usize;
                table[14] = read_stream as *const () as usize;
                table[16] = write_stream as *const () as usize;
                for write in [false, true] {
                    for flags in [0u32, 1, 2, 3, 4, 0x100, u32::MAX] {
                        for mismatch in [0, 1] {
                            for mutate in [0, 1] {
                                for transfer_result in [0xABCD_EF00, 0xABCD_EF80] {
                                    let mut object = [0xA5A5_A5A5u32; 0x60];
                                    let this = object.as_mut_ptr() as u32;
                                    object[0x1C / 4] = flags;
                                    object[0x40 / 4] = table.as_ptr() as u32;
                                    object[0xC0 / 4] = 0xFFFF_FFF0;
                                    let mut expected = object;
                                    let permitted = if write { flags & 2 != 0 } else { flags & 3 != 0 };
                                    if permitted && mutate != 0 {
                                        expected[0xC0 / 4] = 0xFFFF_FFF7;
                                    }
                                    guarded_start([lock_ptr, this, mismatch, transfer_result, mutate, 0, 0, 0], None);
                                    let result = run(
                                        if write {
                                            fc4p_0096e2a7 as *const () as usize
                                        } else {
                                            fc4p_0096e1e5 as *const () as usize
                                        },
                                        this,
                                        &[0x12345678, 0x40, 0xFEDCBA98],
                                    );
                                    let success = permitted && mismatch == 0 && transfer_result as u8 != 0;
                                    assert_eq!(
                                        result,
                                        [(RESULT_EAX & !255) | u32::from(success), RESULT_ECX, RESULT_EDX]
                                    );
                                    assert_eq!(object, expected);
                                    let mut sequence = vec![0x0048_C9F0];
                                    if permitted {
                                        sequence.push(0xFFFF_0030);
                                        if mismatch == 0 {
                                            sequence.push(if write { 0xFFFF_0040 } else { 0xFFFF_0038 });
                                        }
                                    }
                                    sequence.push(0xFFFF_0008);
                                    assert_eq!(targets(), sequence);
                                    let log = calls();
                                    if permitted {
                                        assert_eq!(
                                            log[1],
                                            trace(this + 0x40, table.as_ptr() as u32, RESULT_EDX, &[0x30])
                                        );
                                        if mismatch == 0 {
                                            assert_eq!(
                                                log[2],
                                                trace(
                                                    this + 0x40,
                                                    table.as_ptr() as u32,
                                                    RESULT_EDX,
                                                    &[0x12345678, 0xFEDCBA98]
                                                )
                                            );
                                        }
                                    }
                                    assert_eq!(
                                        log.last().unwrap(),
                                        &trace(lock_ptr, lock_table.as_ptr() as u32, RESULT_EDX, &[])
                                    );
                                }
                            }
                        }
                    }
                }
            }

            #[test]
            fn guarded_directory_initialization_preserves_the_raw_result_byte() {
                let _guard = install(0x0048_C9F0);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32];
                let lock_ptr = lock.as_mut_ptr() as u32;
                let mut table = [0usize; 41];
                table[40] = init_directory as *const () as usize;
                let mut object = [table.as_ptr() as u32, 0xA5A5_A5A5, 0xFEEDBEEF];
                let this = object.as_mut_ptr() as u32;
                for byte in [0u32, 1, 0x80, 0xFF] {
                    guarded_start([lock_ptr, this, 0, 0xDEAD_BE00 | byte, 0, 0, 0, 0], None);
                    assert_eq!(
                        run(fc4p_00975929 as *const () as usize, this, &[0x12345678, 0, u32::MAX]),
                        [(RESULT_EAX & !255) | byte, RESULT_ECX, RESULT_EDX]
                    );
                    assert_eq!(targets(), [0x0048_C9F0, 0xFFFF_00A0, 0xFFFF_0008]);
                    let log = calls();
                    assert_eq!(log[0].arguments, [this + 8]);
                    assert_eq!(
                        log[1],
                        trace(this, table.as_ptr() as u32, RESULT_EDX, &[0x12345678, 0, u32::MAX])
                    );
                    assert_eq!(object, [table.as_ptr() as u32, 0xA5A5_A5A5, 0xFEEDBEEF]);
                }
            }

            #[test]
            fn guarded_record_removal_checks_enabled_then_scans_first_matching_record() {
                let _guard = install(0x0048_C9F0);
                let _erase = crate::testing::fake(0x0093_354C, erase_record as *const () as usize);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32];
                let lock_ptr = lock.as_mut_ptr() as u32;
                for enabled in [0u32, 1, 0x80, 0x100] {
                    for length in [0, 1, 4] {
                        for hit in 0..=length {
                            let mut records = [0xA5A5_A5A5u32; 28];
                            if hit < length {
                                records[hit * 7] = 0x1234_5678;
                            }
                            let begin = records.as_mut_ptr() as u32;
                            let before = records;
                            let mut object = [0xA5A5_A5A5u32; 0x20];
                            object[2] = enabled;
                            object[4] = begin;
                            object[5] = begin + length as u32 * 28;
                            let original = object;
                            let this = object.as_mut_ptr() as u32;
                            guarded_start([lock_ptr, this, 0, 0, 0, 0, 0, 0], None);
                            let removed = enabled as u8 != 0 && hit < length;
                            let result = run(fc4p_0093357b as *const () as usize, this, &[0x1234_5678]);
                            assert_eq!(
                                result,
                                if enabled as u8 == 0 {
                                    [ENTRY_EAX & !255, this, ENTRY_EDX]
                                } else {
                                    [(RESULT_EAX & !255) | u32::from(removed), RESULT_ECX, RESULT_EDX]
                                }
                            );
                            assert_eq!(object, original);
                            assert_eq!(records, before);
                            if enabled as u8 == 0 {
                                assert!(calls().is_empty());
                            } else {
                                assert_eq!(
                                    targets(),
                                    if removed {
                                        vec![0x0048_C9F0, 0x0093_354C, 0xFFFF_0008]
                                    } else {
                                        vec![0x0048_C9F0, 0xFFFF_0008]
                                    }
                                );
                                let log = calls();
                                if removed {
                                    assert_eq!(
                                        log[1],
                                        trace(
                                            this + 0x10,
                                            begin + hit as u32 * 28,
                                            0x1234_5678,
                                            &[begin + hit as u32 * 28]
                                        )
                                    );
                                }
                            }
                        }
                    }
                }
            }

            #[test]
            fn guarded_size_lookup_copies_three_key_words_and_handles_missing_nodes() {
                let _guard = install(0x0048_C9F0);
                let _lookup = crate::testing::fake(0x0096_EEA9, lookup_three_words as *const () as usize);
                let lock_table = [0, 0, release_interface as *const () as usize];
                let mut lock = [lock_table.as_ptr() as u32];
                let lock_ptr = lock.as_mut_ptr() as u32;
                let mut key = [0x1234u32, 0x5678, 0x9ABC];
                let mut node = [0xA5A5_A5A5u32; 6];
                node[5] = 0x87654321;
                let node_ptr = node.as_mut_ptr() as u32;
                for mode in [0, 1, 2, 3, 4, 0x100] {
                    for found in [0, node_ptr] {
                        let mut object = [0xA5A5_A5A5u32; 20];
                        object[7] = mode;
                        object[9] = 0x11223344;
                        let this = object.as_mut_ptr() as u32;
                        let before = object;
                        guarded_start([lock_ptr, this, 0, 0, 0, found, 0, 0], None);
                        assert_eq!(
                            run(fc4p_0096f7cd as *const () as usize, this, &[key.as_mut_ptr() as u32]),
                            [
                                if mode & 3 != 0 && found != 0 {
                                    0x87654321
                                } else {
                                    u32::MAX
                                },
                                RESULT_ECX,
                                RESULT_EDX
                            ]
                        );
                        assert_eq!(object, before);
                        assert_eq!(key, [0x1234, 0x5678, 0x9ABC]);
                        let log = calls();
                        if mode & 3 != 0 {
                            assert_eq!(targets(), [0x0048_C9F0, 0x0096_EEA9, 0xFFFF_0008]);
                            assert_eq!((log[1].this, log[1].eax), (0x11223344, log[1].arguments[0]));
                        } else {
                            assert_eq!(targets(), [0x0048_C9F0, 0xFFFF_0008]);
                        }
                    }
                }
            }

            #[test]
            fn remembering_ranges_requires_two_nonzero_words_and_marks_dirty_after_insert() {
                let _insert = crate::testing::fake(0x0096_F594, remember_pair as *const () as usize);
                for offset in [0, 1, 0x8000_0000, u32::MAX] {
                    for size in [0, 1, 0x8000_0000, u32::MAX] {
                        let mut object = [0xA5u8; 0x160];
                        let this = object.as_mut_ptr() as u32;
                        guarded_start([0, this, 0, 0, 0, 0, offset, size], Some((this, object.len())));
                        let inserted = offset != 0 && size != 0;
                        let result = run(checked_remember_range as *const () as usize, this, &[offset, size]);
                        let expected_eax = if offset == 0 {
                            ENTRY_EAX
                        } else if size == 0 {
                            0
                        } else {
                            BATCH.with(|s| s.borrow().values[2] - 20)
                        };
                        assert_eq!(
                            result,
                            [
                                (expected_eax & !255) | 1,
                                if inserted { RESULT_ECX } else { offset },
                                if inserted { RESULT_EDX } else { ENTRY_EDX }
                            ]
                        );
                        let mut expected = [0xA5u8; 0x160];
                        if inserted {
                            expected[0x158] = 1;
                        }
                        assert_eq!(object, expected);
                        let log = calls();
                        if inserted {
                            assert_eq!(log[0].this, this + 0x14C);
                            assert_eq!(log[0].eax, log[0].arguments[0]);
                            let snapshots = STATE.with(|s| s.borrow().snapshots.clone());
                            assert_eq!(snapshots[0], [0xA5; 0x160]);
                        } else {
                            assert!(log.is_empty());
                        }
                    }
                }
            }

            #[test]
            fn title_alignment_groups_valid_values_skips_unchanged_and_rejects_out_of_range() {
                let _refresh = crate::testing::fake(0x009C_1E6D, refresh_style as *const () as usize);
                let mut table = [0usize; 93];
                table[92] = refresh_title as *const () as usize;
                for previous in [0, 5, 8, 9, u32::MAX] {
                    for alignment in [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 256, u32::MAX] {
                        let mut object = [0xA5A5_A5A5u32; 80];
                        object[0] = table.as_ptr() as u32;
                        let base = object.as_mut_ptr() as u32;
                        let this = base + 0xD8;
                        object[(0xD8 + 0x2C) / 4] = previous;
                        let mut expected = object;
                        let changed = previous != alignment && alignment <= 8;
                        if changed {
                            expected[(0xD8 + 0x2C) / 4] = alignment;
                            expected[(0xD8 + 0x38) / 4] = alignment / 3;
                        }
                        start(None, [0; 8]);
                        let result = run(fc4p_009c213e as *const () as usize, this, &[alignment]);
                        assert_eq!(object, expected);
                        if changed {
                            assert_eq!(result, [(RESULT_EAX & !255) | 1, RESULT_ECX, RESULT_EDX]);
                            assert_eq!(targets(), [0x009C_1E6D, 0xFFFF_0170]);
                            assert_eq!(
                                calls(),
                                [
                                    trace(base, alignment, 2, &[]),
                                    trace(base, table.as_ptr() as u32, RESULT_EDX, &[])
                                ]
                            );
                        } else {
                            assert_eq!(
                                result,
                                [
                                    (alignment & !255) | u32::from(previous == alignment),
                                    this,
                                    if previous == alignment { ENTRY_EDX } else { 2 }
                                ]
                            );
                            assert!(calls().is_empty());
                        }
                    }
                }
            }

            #[test]
            fn hash_iteration_keeps_bucket_order_with_head_insertion() {
                // The shared container's existing iteration fixture uses colliding keys 5 and 198.
                let mut nodes = [[0u32; 4]; 4];
                let pointers = core::array::from_fn::<_, 4, _>(|i| nodes[i].as_mut_ptr() as u32);
                for (node, key) in nodes.iter_mut().zip([5, 198, 1, 400]) {
                    node[3] = key;
                }
                nodes[1][0] = pointers[0];
                let mut buckets = [0u32; 193];
                buckets[1] = pointers[2];
                buckets[5] = pointers[1];
                buckets[14] = pointers[3];
                let begin = buckets.as_mut_ptr() as u32;
                let mut table = [0, begin, begin + 193 * 4];
                let mut iterator = [pointers[2], table.as_mut_ptr() as u32];
                let mut keys = Vec::new();
                while iterator[0] != 0 {
                    // SAFETY: every iterator node belongs to the live fixture.
                    unsafe {
                        keys.push(word(iterator[0], 12));
                    }
                    // SAFETY: the fixture's link word is readable.
                    let next = unsafe { word(iterator[0], 0) };
                    iterator[0] = if next != 0 {
                        next
                    } else {
                        run(fc4p_0097c1ea as *const () as usize, iterator.as_mut_ptr() as u32, &[])[0]
                    };
                }
                assert_eq!(keys, [1, 198, 5, 400]);
                assert_eq!(nodes[1][0], pointers[0]);
            }

            #[test]
            fn reverse_word_search_covers_every_unrolled_and_tail_position() {
                for length in 0usize..=24 {
                    for hit in 0..=length {
                        let mut words = [0xABCDu16; 26];
                        if hit < length {
                            words[hit] = 0x5678;
                        }
                        let before = words;
                        let begin = words.as_ptr() as u32;
                        let end = begin + length as u32 * 2;
                        let mut output = 0xA5A5_A5A5;
                        let out = &mut output as *mut u32 as u32;
                        let result = invoke(
                            fc4p_00918731 as *const () as usize,
                            0x1234,
                            &[out, end, begin, 0xFFFF_5678, 0xDEAD_BEEF],
                            ENTRY_EAX,
                            ENTRY_EDX,
                            true,
                        );
                        assert_eq!(
                            output,
                            if hit < length {
                                begin + (hit as u32 + 1) * 2
                            } else {
                                begin
                            }
                        );
                        let tail = length % 4;
                        let (ecx, edx) = if hit < length && hit >= tail {
                            let group = (length - 1 - hit) / 4;
                            let ecx = if (length - 1 - hit) % 4 == 0 {
                                end - group as u32 * 8
                            } else {
                                begin + hit as u32 * 2
                            };
                            (ecx, (length / 4 - group) as u32)
                        } else if hit < length {
                            (
                                begin + (hit as u32 + 1) * 2,
                                if tail == 1 { 0 } else { begin + hit.max(1) as u32 * 2 },
                            )
                        } else {
                            (
                                begin + u32::from(tail != 0) * 2,
                                match tail {
                                    0 => 0xFFFF_FFFD,
                                    1 => 0,
                                    _ => begin + 2,
                                },
                            )
                        };
                        assert_eq!(result, [out, ecx, edx], "length={length} hit={hit}");
                        assert_eq!(words, before);
                    }
                }
                let mut words = [7u16; 9];
                let mut output = 0;
                let begin = words.as_mut_ptr() as u32;
                invoke(
                    fc4p_00918731 as *const () as usize,
                    0,
                    &[&mut output as *mut u32 as u32, begin + 18, begin, 7],
                    0,
                    0,
                    true,
                );
                assert_eq!(output, begin + 18);
                for (begin, end) in [(0x1000u32, 0x0FFE), (0, 0x8000_0000), (0x8000_0000, 0)] {
                    let out = &mut output as *mut u32 as u32;
                    let result = invoke(
                        fc4p_00918731 as *const () as usize,
                        0x1234,
                        &[out, end, begin, 0x5678],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true,
                    );
                    assert_eq!(
                        result,
                        [
                            out,
                            end,
                            (((end.wrapping_sub(begin) as i32) >> 1) as u32).wrapping_sub(3)
                        ]
                    );
                    assert_eq!(output, begin);
                }
            }

            #[test]
            fn packed_color_blend_preserves_wrapping_math_and_register_results() {
                // Independent widened arithmetic retains the low 32 bits at each packed stage.
                fn low(x: i128) -> u32 {
                    (x & 0xFFFF_FFFF) as u32
                }
                for source in [0, 1, 0xFF, 0xFF00, 0x1234_5678, 0xFF00_FF00, u32::MAX] {
                    for destination in [0, 0xFF, 0x1020_3040, 0xFF00_FF00, u32::MAX] {
                        for weight in [0, 1, 127, 128, 255, 256, 257, 0x8000_0000, u32::MAX] {
                            let rb = i128::from(destination & 0x00FF_00FF);
                            let high = i128::from(destination & 0xFF00_FF00);
                            let product =
                                low((i128::from(source & 0x00FF_00FF) - rb) * i128::from(weight) + 0x0080_0080);
                            let a = low(i128::from(product >> 8) + rb);
                            let d = low(
                                (i128::from((source >> 8) & 0xFFFF_00FF) - (high >> 8)) * i128::from(weight)
                                    + high
                                    + 0xFF80_0080,
                            );
                            let c = low(
                                (i128::from((source >> 8) & 0x00FF_00FF) - (high >> 8)) * i128::from(weight)
                                    + high
                                    + 0x0080_0080,
                            );
                            let result = invoke(
                                fc4p_009855d0 as *const () as usize,
                                0x1234,
                                &[source, destination, weight],
                                ENTRY_EAX,
                                ENTRY_EDX,
                                true,
                            );
                            assert_eq!(result, [((a ^ d) & 0x00FF_00FF) ^ c, c, d]);
                        }
                    }
                }
            }

            #[test]
            fn repeated_record_construction_keeps_order_source_and_final_pop() {
                let _fake = install(0x009A_7610);
                for count in [0, 1, 2, 7] {
                    start(None, [0; 8]);
                    let result = invoke(
                        fc4p_009334f1 as *const () as usize,
                        0x9876,
                        &[0x1234_5000, count, 0x8765_4321, 0xABCDEF],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true,
                    );
                    assert_eq!(
                        result,
                        [
                            0x1234_5000 + 28 * count,
                            if count == 0 { 0x9876 } else { 0x8765_4321 },
                            if count == 0 { ENTRY_EDX } else { RESULT_EDX }
                        ]
                    );
                    let log = calls();
                    assert_eq!(targets(), vec![0x009A_7610; count as usize]);
                    for (i, call) in log.iter().enumerate() {
                        assert_eq!(call.arguments, [0x1234_5000 + i as u32 * 28, 0x8765_4321]);
                        assert_eq!(call.this, if i == 0 { 0x9876 } else { 0x8765_4321 });
                    }
                }
            }

            #[test]
            fn record_copy_and_destruction_wrappers_forward_tags_and_cdecl_cleanup() {
                let _copy = crate::testing::fake(0x0090_91B2, copy_records as *const () as usize);
                let _destroy = crate::testing::fake(0x0093_B776, destroy_elements as *const () as usize);
                for (entry, target, extra) in [
                    (fc4p_0093352f as *const () as usize, 0x0090_91B2, true),
                    (fc4p_00981543 as *const () as usize, 0x0093_B776, false),
                ] {
                    start(None, [0; 8]);
                    assert_eq!(
                        invoke(
                            entry,
                            0x8765_4321,
                            &[0x1000, 0x1000, 0x2000, 0x3000],
                            ENTRY_EAX,
                            ENTRY_EDX,
                            true
                        ),
                        [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                    );
                    let log = calls();
                    assert_eq!(targets(), [target]);
                    let call = &log[0];
                    assert_eq!((call.this, call.edx), (0x8765_4321, ENTRY_EDX));
                    assert_eq!(call.arguments[..2], [0x1000, 0x1000]);
                    let tag_index = if extra { 3 } else { 2 };
                    assert_eq!(call.eax, call.arguments[tag_index]);
                    if extra {
                        assert_eq!(call.arguments[2], 0x2000);
                        assert_eq!(call.arguments[4], 0);
                    }
                }
            }

            #[test]
            fn record_erasure_skips_empty_shift_and_reloads_mutated_end() {
                unsafe fn hook(r: &mut Registers, _: u32) {
                    let object = BATCH.with(|s| s.borrow().values[0]);
                    // SAFETY: the vector is live test storage; emulate a helper changing its end.
                    unsafe {
                        put_word(object, 4, 0x9000);
                    }
                    results(r);
                }
                let _fake = crate::testing::fake(0x0093_352F, move_records as *const () as usize);
                for last in [true, false] {
                    let mut vector = [0x1000u32, if last { 0x1038 } else { 0x10A8 }, 0x10FF, 0xA5A5_A5A5];
                    let this = vector.as_mut_ptr() as u32;
                    start(Some(hook), [this, 0, 0, 0, 0, 0, 0, 0]);
                    let result = run(fc4p_0093354c as *const () as usize, this, &[0x101C]);
                    assert_eq!(result[0], 0x101C);
                    assert_eq!(
                        vector,
                        [0x1000, if last { 0x101C } else { 0x8FE4 }, 0x10FF, 0xA5A5_A5A5]
                    );
                    let log = calls();
                    if last {
                        assert!(log.is_empty());
                        assert_eq!(result, [0x101C, 0x1038, ENTRY_EDX]);
                    } else {
                        assert_eq!(log[0].arguments[..3], [0x1038, 0x10A8, 0x101C]);
                        assert_eq!(log[0].edx, log[0].arguments[3]);
                        assert_eq!(&result[1..], &[RESULT_ECX, RESULT_EDX]);
                    }
                }
                let mut vector = [0x1000, 0x1100, 0x1200, 0xA5A5_A5A5];
                let this = vector.as_mut_ptr() as u32;
                start(None, [0; 8]);
                assert_eq!(
                    run(fc4p_009335ce as *const () as usize, this, &[0x101C, 0x1054]),
                    [0x101C, RESULT_ECX, RESULT_EDX]
                );
                assert_eq!(vector, [0x1000, RESULT_EAX, 0x1200, 0xA5A5_A5A5]);
                let log = calls();
                assert_eq!(log[0].arguments[..3], [0x1054, 0x1100, 0x101C]);
                assert_eq!(log[0].eax, log[0].arguments[3]);
            }

            #[test]
            fn optional_copy_preserves_entry_registers_for_a_null_destination() {
                let _fake = crate::testing::fake(0x0094_0627, retained_copy as *const () as usize);
                for destination in [0, 0x1234_5678] {
                    start(None, [0; 8]);
                    let result = run(fc4p_00940613 as *const () as usize, 0x8765_4321, &[destination]);
                    if destination == 0 {
                        assert_eq!(result, [ENTRY_EAX, 0x8765_4321, ENTRY_EDX]);
                        assert!(calls().is_empty());
                    } else {
                        assert_eq!(result, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                        assert_eq!(calls(), [trace(destination, ENTRY_EAX, ENTRY_EDX, &[0x8765_4321])]);
                    }
                }
            }

            #[test]
            fn list_erasure_unlinks_before_reset_and_free_and_returns_the_next_node() {
                let mut next = [0xA5A5_A5A5u32; 3];
                let mut previous = [0xA5A5_A5A5u32; 3];
                let next_ptr = next.as_mut_ptr() as u32;
                let previous_ptr = previous.as_mut_ptr() as u32;
                let mut node = [next_ptr, previous_ptr, 0xCAFEBABE, 0x11223344];
                let node_ptr = node.as_mut_ptr() as u32;
                let mut output = [0xA5A5_A5A5; 2];
                let out = output.as_mut_ptr() as u32;
                let _reset = crate::testing::fake(0x0093_AB29, reset_value as *const () as usize);
                let _free = install(0x0090_CF63);
                start(None, [0; 8]);
                assert_eq!(
                    run(fc4p_00968c53 as *const () as usize, 0x11223344, &[out, node_ptr]),
                    [out, node_ptr, RESULT_EDX]
                );
                assert_eq!(previous, [next_ptr, 0xA5A5_A5A5, 0xA5A5_A5A5]);
                assert_eq!(next, [0xA5A5_A5A5, previous_ptr, 0xA5A5_A5A5]);
                assert_eq!(output, [next_ptr, 0xA5A5_A5A5]);
                assert_eq!(node, [next_ptr, previous_ptr, 0xCAFEBABE, 0x11223344]);
                assert_eq!(targets(), [0x0093_AB29, 0x0090_CF63]);
                assert_eq!(
                    calls(),
                    [
                        trace(node_ptr + 8, previous_ptr, ENTRY_EDX, &[0]),
                        trace(RESULT_ECX, RESULT_EAX, RESULT_EDX, &[node_ptr])
                    ]
                );
            }

            #[test]
            fn node_allocation_uses_exact_sizes_zeroes_only_link_and_calls_value_constructor() {
                unsafe fn hook(r: &mut Registers, address: u32) {
                    let node = BATCH.with(|s| s.borrow().values[0]);
                    if address == 0x0090_CF54 {
                        results(r);
                        r.eax = node;
                    } else {
                        // SAFETY: every constructor receives the allocated test node with its first link cleared.
                        unsafe {
                            assert_eq!(word(node, 0), 0);
                        }
                        results(r);
                    }
                }
                let _allocate = crate::testing::fake(0x0090_CF54, allocate_storage as *const () as usize);
                let _pair = crate::testing::fake(0x0099_7792, copy_pair_value as *const () as usize);
                let _hash = crate::testing::fake(0x0096_E78B, copy_hash as *const () as usize);
                for (entry, size, target) in [
                    (fc4p_00968cb6 as *const () as usize, 12, 0x0099_7792),
                    (fc4p_0096edd3 as *const () as usize, 24, 0x0096_E78B),
                ] {
                    let mut node = [0xA5A5_A5A5u32; 8];
                    let pointer = node.as_mut_ptr() as u32;
                    start(Some(hook), [pointer, 0, 0, 0, 0, 0, 0, 0]);
                    assert_eq!(run(entry, 0x12345678, &[0x87654321]), [pointer, RESULT_ECX, RESULT_EDX]);
                    assert_eq!(
                        node,
                        [
                            0,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5,
                            0xA5A5_A5A5
                        ]
                    );
                    assert_eq!(targets(), [0x0090_CF54, target]);
                    let log = calls();
                    assert_eq!(log[0], trace(0x12345678, ENTRY_EAX, ENTRY_EDX, &[size]));
                    assert_eq!(
                        log[1],
                        if size == 12 {
                            trace(RESULT_ECX, pointer + 4, RESULT_EDX, &[pointer + 4, 0x87654321])
                        } else {
                            trace(pointer + 4, pointer, RESULT_EDX, &[0x87654321])
                        }
                    );
                }
            }

            #[test]
            fn tree_constructor_reuses_one_tag_and_returns_this() {
                let _fake = crate::testing::fake(0x0093_7FAD, tree_tags as *const () as usize);
                start(None, [0; 8]);
                assert_eq!(
                    run(fc4p_0096f57b as *const () as usize, 0x1234_5678, &[]),
                    [0x1234_5678, RESULT_ECX, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(log[0].arguments, [log[0].eax, log[0].eax]);
                assert_eq!((log[0].this, log[0].edx), (0x1234_5678, ENTRY_EDX));
            }

            #[test]
            fn tree_pair_result_copies_exactly_five_bytes_and_retains_ecx_upper_bytes() {
                unsafe fn hook(r: &mut Registers, _: u32) {
                    let flag = BATCH.with(|s| s.borrow().values[0]);
                    // SAFETY: the insertion wrapper supplies two writable result words.
                    unsafe {
                        put_word(r.argument(0), 0, 0x8765_4321);
                        put_word(r.argument(0), 4, 0xFEDC_BA00 | flag);
                    }
                    results(r);
                }
                let _fake = crate::testing::fake(0x0096_F12D, insert_pair as *const () as usize);
                for flag in [0, 1, 0x80, 0xFF] {
                    let mut output = [0xA5u8; 12];
                    let out = output.as_mut_ptr() as u32;
                    start(Some(hook), [flag, 0, 0, 0, 0, 0, 0, 0]);
                    assert_eq!(
                        run(fc4p_0096f594 as *const () as usize, 0x10203040, &[out, 0x55667788]),
                        [out, 0x8765_4300 | flag, RESULT_EDX]
                    );
                    assert_eq!(&output[..5], &[0x21, 0x43, 0x65, 0x87, flag as u8]);
                    assert_eq!(&output[5..], &[0xA5; 7]);
                    let log = calls();
                    assert_eq!(log[0].eax, log[0].arguments[0]);
                    assert_eq!(log[0].arguments[1], 0x55667788);
                }
            }

            #[test]
            fn filled_vector_constructor_reloads_storage_and_sets_only_end() {
                unsafe fn hook(r: &mut Registers, address: u32) {
                    // SAFETY: the initializer owns these three vector fields in test storage.
                    unsafe {
                        if address == 0x009D_16D9 {
                            put_word(r.ecx, 0, 0x2000);
                            put_word(r.ecx, 8, 0x3000);
                        }
                    }
                    results(r);
                }
                let _init = crate::testing::fake(0x009D_16D9, initialize_vector as *const () as usize);
                let _fill = crate::testing::fake(0x0041_86F0, fill_vector as *const () as usize);
                for count in [0, 1, 5, u32::MAX] {
                    let mut vector = [0xA5A5_A5A5u32; 4];
                    let this = vector.as_mut_ptr() as u32;
                    start(Some(hook), [0; 8]);
                    assert_eq!(
                        run(
                            fc4p_0097c01d as *const () as usize,
                            this,
                            &[count, 0xDEADBEEF, 0xAABBCCDD]
                        ),
                        [this, RESULT_ECX, RESULT_EDX]
                    );
                    assert_eq!(vector, [0x2000, RESULT_EAX, 0x3000, 0xA5A5_A5A5]);
                    assert_eq!(
                        calls(),
                        [
                            trace(this, ENTRY_EAX, ENTRY_EDX, &[count, 0xAABBCCDD]),
                            trace(RESULT_ECX, 0x2000, RESULT_EDX, &[0x2000, count, 0xDEADBEEF])
                        ]
                    );
                    assert_eq!(targets(), [0x009D_16D9, 0x0041_86F0]);
                }
            }

            #[test]
            fn hash_lookup_follows_chains_and_tests_only_low_compare_byte() {
                unsafe fn hook(r: &mut Registers, _: u32) {
                    let match_value = BATCH.with(|s| s.borrow().values[0]);
                    let matches = r.ecx == match_value;
                    results(r);
                    r.eax = 0xAABB_CC00 | u32::from(matches);
                }
                let _fake = crate::testing::fake(0x0043_1180, compare_value as *const () as usize);
                let mut third = [0u32; 6];
                let c = third.as_mut_ptr() as u32;
                let mut second = [c, 0, 0, 0, 0, 0];
                let b = second.as_mut_ptr() as u32;
                let mut first = [b, 0, 0, 0, 0, 0];
                let a = first.as_mut_ptr() as u32;
                let mut buckets = [0, a, 0, 0, 0];
                let begin = buckets.as_mut_ptr() as u32;
                let mut table = [0, begin, begin + 20, begin + 20, 3];
                let this = table.as_mut_ptr() as u32;
                for hash in [1, 6, 11, u32::MAX - 4] {
                    let mut key = [0, 0, hash];
                    let value = key.as_mut_ptr() as u32;
                    for matched in [0, a, b, c] {
                        start(Some(hook), [matched.wrapping_add(4), 0, 0, 0, 0, 0, 0, 0]);
                        let result = run(fc4p_0097c049 as *const () as usize, this, &[value]);
                        assert_eq!(result[0], matched);
                        assert_eq!(&result[1..], &[RESULT_ECX, RESULT_EDX]);
                        let log = calls();
                        let expected = if matched == a {
                            1
                        } else if matched == b {
                            2
                        } else {
                            3
                        };
                        assert_eq!(log.len(), expected);
                        assert_eq!(log[0], trace(a + 4, begin, hash % 5, &[value]));
                        for (i, call) in log.iter().enumerate() {
                            assert_eq!(call.this, [a + 4, b + 4, c + 4][i]);
                            assert_eq!(call.arguments, [value]);
                        }
                    }
                }
                let mut key = [0, 0, 2];
                start(None, [0; 8]);
                assert_eq!(
                    run(fc4p_0097c049 as *const () as usize, this, &[key.as_mut_ptr() as u32]),
                    [0, this, 2]
                );
                assert!(calls().is_empty());
            }

            #[test]
            fn next_bucket_scan_stops_at_first_occupied_and_never_wraps() {
                let mut buckets = [0u32; 7];
                let begin = buckets.as_mut_ptr() as u32;
                let mut table = [0, begin, begin + 28];
                let mut node = [0u32; 4];
                let mut iterator = [node.as_mut_ptr() as u32, table.as_mut_ptr() as u32];
                let this = iterator.as_mut_ptr() as u32;
                for hash in [0, 1, 5, 6, 7, 99, u32::MAX] {
                    node[3] = hash;
                    for occupied in 0..=7 {
                        buckets.fill(0);
                        if occupied < 7 {
                            buckets[occupied] = 0x12345678;
                        }
                        let start = hash % 7 + 1;
                        let hit = occupied < 7 && occupied as u32 >= start;
                        assert_eq!(
                            run(fc4p_0097c1ea as *const () as usize, this, &[]),
                            [
                                if hit { 0x12345678 } else { 0 },
                                this,
                                if hit { occupied as u32 } else { 7 }
                            ]
                        );
                    }
                }
            }

            #[test]
            fn hash_iterator_erase_copies_both_words_before_calling() {
                unsafe fn hook(r: &mut Registers, _: u32) {
                    // SAFETY: the wrapper supplies a live two-word iterator.
                    unsafe {
                        let p = r.argument(0);
                        assert_eq!(r.eax, p);
                        assert_eq!((word(p, 0), word(p, 4)), (0x12345678, 0x87654321));
                        put_word(p, 0, 0);
                        put_word(p, 4, 0);
                    }
                    results(r);
                }
                let _fake = crate::testing::fake(0x0097_C14C, erase_iterator as *const () as usize);
                start(Some(hook), [0; 8]);
                assert_eq!(
                    run(
                        fc4p_0097c221 as *const () as usize,
                        0x11223344,
                        &[0x12345678, 0x87654321]
                    ),
                    [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                );
                assert_eq!(targets(), [0x0097_C14C]);
            }

            #[test]
            fn tree_value_lookup_writes_only_hits_and_keeps_partial_al_results() {
                unsafe fn hook(r: &mut Registers, _: u32) {
                    let found = BATCH.with(|s| s.borrow().values[0]);
                    // SAFETY: the search key points at the original writable argument word.
                    unsafe {
                        assert_eq!(word(r.argument(0), 0), 0xFEEDBEEF);
                        put_word(r.argument(0), 0, 0);
                    }
                    results(r);
                    r.eax = found;
                }
                let _fake = crate::testing::fake(0x0094_C8F5, find_key as *const () as usize);
                let mut tree = [0x1234_5600u32; 2];
                let this = tree.as_mut_ptr() as u32;
                for found in [tree[0], 0x1234_56F0] {
                    let mut output = [0xA5A5_A5A5u32; 2];
                    let out = output.as_mut_ptr() as u32;
                    start(Some(hook), [found, 0, 0, 0, 0, 0, 0, 0]);
                    let hit = found != tree[0];
                    assert_eq!(
                        run(fc4p_009a94a6 as *const () as usize, this, &[0xFEEDBEEF, out]),
                        [
                            if hit {
                                (found.wrapping_add(20) & !255) | 1
                            } else {
                                found & !255
                            },
                            if hit { out } else { RESULT_ECX },
                            RESULT_EDX
                        ]
                    );
                    assert_eq!(output, [if hit { found + 20 } else { 0xA5A5_A5A5 }, 0xA5A5_A5A5]);
                    assert_eq!(tree, [0x1234_5600; 2]);
                    let log = calls();
                    assert_eq!(log[0].eax, log[0].arguments[0]);
                }
            }

            #[test]
            fn title_window_lifetime_sets_tables_and_preserves_padding_and_base_call_order() {
                unsafe fn hook(r: &mut Registers, address: u32) {
                    // SAFETY: the first destructor must see both title tables already installed.
                    unsafe {
                        if address == 0x0099_D2FE {
                            assert_eq!(word(r.ecx, 0), 0x00AE_20A0);
                            assert_eq!(word(r.ecx, 0xD8), 0x00AE_2050);
                            put_word(r.ecx, 4, 0xFEEDBEEF);
                        } else if address == 0x0099_D938 {
                            put_word(r.ecx, 4, 0xCAFEBABE);
                        }
                    }
                    results(r);
                }
                let _d1 = crate::testing::fake(0x0099_D2FE, destroy_title as *const () as usize);
                let _d2 = crate::testing::fake(0x0099_E1A2, destroy_base as *const () as usize);
                let _c = crate::testing::fake(0x0099_D938, construct_base as *const () as usize);
                for construct in [false, true] {
                    let mut object = [0xA5u8; 0xF4];
                    let this = object.as_mut_ptr() as u32;
                    start(Some(hook), [0; 8]);
                    let result = run(
                        if construct {
                            fc4p_009cd842 as *const () as usize
                        } else {
                            fc4p_009cd1b2 as *const () as usize
                        },
                        this,
                        &[],
                    );
                    let mut expected = [0xA5u8; 0xF4];
                    expected[..4].copy_from_slice(&0x00AE_20A0u32.to_le_bytes());
                    expected[4..8]
                        .copy_from_slice(&if construct { 0xCAFEBABEu32 } else { 0xFEEDBEEFu32 }.to_le_bytes());
                    expected[0xD8..0xDC].copy_from_slice(&0x00AE_2050u32.to_le_bytes());
                    if construct {
                        expected[0xDC..0xDE].fill(0);
                        expected[0xE0..0xF0].fill(0);
                    }
                    assert_eq!(object, expected);
                    assert_eq!(
                        result,
                        [if construct { this } else { RESULT_EAX }, RESULT_ECX, RESULT_EDX]
                    );
                    assert_eq!(
                        targets(),
                        if construct {
                            vec![0x0099_D938]
                        } else {
                            vec![0x0099_D2FE, 0x0099_E1A2]
                        }
                    );
                    let log = calls();
                    assert_eq!(log[0], trace(this, ENTRY_EAX, ENTRY_EDX, &[]));
                    if !construct {
                        assert_eq!(log[1], trace(this, RESULT_EAX, RESULT_EDX, &[]));
                    }
                }
            }

            #[test]
            fn list_front_erasure_and_append_keep_the_original_output_destinations() {
                unsafe fn hook(r: &mut Registers, address: u32) {
                    // SAFETY: both wrappers pass a writable iterator output word.
                    let output = unsafe { r.argument(0) };
                    if address == 0x0094_9259 {
                        let entry_stack = BATCH.with(|s| s.borrow().values[2]);
                        assert_eq!(output, entry_stack - 4);
                    }
                    unsafe {
                        put_word(output, 0, 0x55667788);
                    }
                    results(r);
                    if address == 0x0094_9259 { r.eax = output; }
                }
                let _erase = crate::testing::fake(0x0094_9259, erase_first as *const () as usize);
                let _insert = install(0x0089_7C8D);
                let mut sentinel = [0x12345678u32, 0xA5A5_A5A5];
                let mut list = [sentinel.as_mut_ptr() as u32, 0xA5A5_A5A5];
                let this = list.as_mut_ptr() as u32;
                start(Some(hook), [0; 8]);
                let result = run(checked_erase_first as *const () as usize, this, &[]);
                let entry_stack = BATCH.with(|s| s.borrow().values[2]);
                assert_eq!(result, [entry_stack - 4, 0x55667788, RESULT_EDX]);
                let log = calls();
                assert_eq!(
                    log[0],
                    trace(
                        this,
                        log[0].arguments[0],
                        0x12345678,
                        &[log[0].arguments[0], 0x12345678]
                    )
                );
                start(Some(hook), [0; 8]);
                assert_eq!(
                    run(fc4p_009d74d9 as *const () as usize, this, &[0xDEADBEEF]),
                    [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(log[0].arguments, [log[0].eax, 0x12345678, 0xDEADBEEF]);
                assert_eq!((log[0].this, log[0].edx), (this, 0x12345678));
                assert_eq!(sentinel, [0x12345678, 0xA5A5_A5A5]);
                assert_eq!(list[1], 0xA5A5_A5A5);
            }

            #[test]
            fn tree_hint_wrapper_forwards_three_words_and_returns_original_output() {
                let _fake = crate::testing::fake(0x0098_1404, tree_hint as *const () as usize);
                start(None, [0; 8]);
                assert_eq!(
                    run(
                        fc4p_009dc50a as *const () as usize,
                        0x11223344,
                        &[0x12345678, 0x87654321, 0xFEDCBA98]
                    ),
                    [0x12345678, RESULT_ECX, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(log[0].arguments, [0x12345678, 0x87654321, 0xFEDCBA98]);
                assert_eq!((log[0].this, log[0].edx), (0x11223344, 0x87654321));
                assert_eq!(targets(), [0x0098_1404]);
            }
        }

        #[test]
        fn list_insertion_uses_the_original_argument_as_output() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                // SAFETY: the insertion receives a writable argument slot.
                unsafe {
                    assert_eq!(r.eax, r.argument(0));
                    assert_eq!(word(r.eax, 0), r.argument(2));
                    assert_eq!(r.edx, r.argument(1));
                    put_word(r.eax, 0, 0xFEED_BEEF);
                }
                results(r);
            }
            let _fake = install(0x0089_7C8D);
            for value in [0, 1, 0x8000_0000, u32::MAX] {
                start(Some(hook), [0; 8]);
                let mut object = [0x1234_5678, 0xA5A5_A5A5];
                let before = object;
                let this = object.as_mut_ptr() as u32;
                assert_eq!(
                    run(fc4p_008f6e80 as *const () as usize, this, &[value]),
                    [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(log.len(), 1);
                assert_eq!(
                    log[0],
                    trace(
                        this,
                        log[0].arguments[0],
                        object[0],
                        &[log[0].arguments[0], object[0], value]
                    )
                );
                assert_eq!(object, before);
                assert_eq!(targets(), [0x0089_7C8D]);
            }
        }

        #[test]
        fn word_append_checks_capacity_null_storage_and_aliasing_before_growth() {
            let _fake = install(0x0099_4D77);
            for alias in [false, true] {
                start(None, [0; 8]);
                let mut storage = [0xA5A5_A5A5u32; 3];
                let mut vector = [0xABCD, storage.as_mut_ptr() as u32, 0x1234_5678];
                let this = vector.as_mut_ptr() as u32;
                if alias {
                    vector[1] = this + 4;
                }
                let destination = vector[1];
                let mut input = 0x7654_3210u32;
                let result = run(
                    fc4p_008ff785 as *const () as usize,
                    this,
                    &[&mut input as *mut u32 as u32],
                );
                assert_eq!(result, [destination, this, input]);
                assert_eq!(
                    vector,
                    [0xABCD, if alias { input + 4 } else { destination + 4 }, 0x1234_5678]
                );
                assert_eq!(
                    storage,
                    if alias {
                        [0xA5A5_A5A5; 3]
                    } else {
                        [input, 0xA5A5_A5A5, 0xA5A5_A5A5]
                    }
                );
                assert!(calls().is_empty());
            }
            start(None, [0; 8]);
            let mut null_vector = [0xABCD, 0, 16];
            let this = null_vector.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_008ff785 as *const () as usize, this, &[0]),
                [0, this, ENTRY_EDX]
            );
            assert_eq!(null_vector, [0xABCD, 4, 16]);

            unsafe fn growth(r: &mut Registers, _: u32) {
                // SAFETY: the tag points to the high byte of the original input argument.
                unsafe {
                    assert_eq!((r.edx as *const u8).read(), (r.argument(1) >> 24) as u8);
                    assert_eq!(r.argument(2), r.edx);
                    put_word(r.ecx, 4, 0x1234_5678);
                    put_word(r.ecx, 8, 0x2345_6789);
                }
                results(r);
            }
            for end in [0, 0x8765_4321] {
                start(Some(growth), [0; 8]);
                let mut vector = [0xABCD, end, end];
                let this = vector.as_mut_ptr() as u32;
                assert_eq!(
                    run(fc4p_008ff785 as *const () as usize, this, &[0xFEDC_BA98]),
                    [RESULT_EAX, RESULT_ECX, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(log.len(), 1);
                assert_eq!(
                    log[0],
                    trace(this, end, log[0].edx, &[end, 0xFEDC_BA98, log[0].edx, 1, 1])
                );
                assert_eq!(vector, [0xABCD, 0x1234_5678, 0x2345_6789]);
            }
        }

        unsafe fn resource_hook(r: &mut Registers, address: u32) {
            // SAFETY: these fakes operate only on the test's vectors, resource nodes and callback mutations.
            unsafe {
                let mut result = RESULT_EAX;
                if address == 0x009A_1ADD {
                    put_word(r.ecx, 4, r.argument(0));
                } else if address == 0x0040_D4C0 {
                    result = word(r.argument(0), 0);
                }
                results(r);
                r.eax = result;
            }
        }

        #[test]
        fn document_shutdown_covers_optional_services_vectors_and_both_resource_paths() {
            let _fakes = install_many(&[0x009A_1ADD, 0x0040_D4C0, 0x008F_FD4F]);
            let mut vt = [0usize; 29];
            vt[0] = delete_value as *const () as usize;
            vt[2] = release_interface as *const () as usize;
            vt[0x58 / 4] = release_resource as *const () as usize;
            vt[0x70 / 4] = delete_window as *const () as usize;
            let mut service = [vt.as_ptr() as u32, 0xA5A5_A5A5];
            let interface = service.as_mut_ptr() as u32;
            for mask in 0..32 {
                for populated in [false, true] {
                    start(Some(resource_hook), [0; 8]);
                    let mut object = [0xA5u8; 0x1B0];
                    let this = object.as_mut_ptr() as u32;
                    let mut first = [interface, 0, interface];
                    let mut second = [0, interface, 0];
                    let mut header = [0u32; 3];
                    let sentinel = header.as_mut_ptr() as u32;
                    let mut nodes = [[0u32; 14]; 2];
                    let p0 = nodes[0].as_mut_ptr() as u32;
                    let p1 = nodes[1].as_mut_ptr() as u32;
                    nodes[0][0] = p1;
                    nodes[1][0] = sentinel;
                    nodes[0][9] = interface;
                    nodes[1][9] = if mask & 8 != 0 { 0 } else { interface };
                    nodes[1][13] = 0x9876_5432;
                    header[2] = if populated { p0 } else { sentinel };
                    let nodes_before = nodes;
                    let header_before = header;
                    object[0x18] = 0xFF;
                    // SAFETY: all fields are within the test object.
                    unsafe {
                        for (bit, offset) in [0x14, 0xB8, 0x1A4, 0x30, 0x2C].into_iter().enumerate() {
                            put_word(this, offset, if mask & (1 << bit) != 0 { interface } else { 0 });
                        }
                        for (offset, ptr) in [(0xAC, first.as_mut_ptr() as u32), (0xBC, second.as_mut_ptr() as u32)] {
                            put_word(this, offset, ptr);
                            put_word(this, offset + 4, ptr + if populated { 12 } else { 0 });
                        }
                        put_word(this, 0x198, sentinel);
                    }
                    let mut expected = object;
                    expected[0x18] = 0;
                    for offset in [0x14, 0xB8, 0x1A4, 0x30, 0x2C] {
                        expected[offset..offset + 4].fill(0);
                    }
                    expected[0xB0..0xB4].copy_from_slice(&(first.as_ptr() as u32).to_le_bytes());
                    expected[0xC0..0xC4].copy_from_slice(&(second.as_ptr() as u32).to_le_bytes());
                    STATE.with(|s| s.borrow_mut().watch = Some((this, object.len())));
                    assert_eq!(
                        run(fc4p_00900357 as *const () as usize, this, &[]),
                        [
                            (RESULT_EAX & !255) | 1,
                            if mask & 16 != 0 { RESULT_ECX } else { 0 },
                            RESULT_EDX
                        ]
                    );
                    let mut expected_calls: Vec<(u32, u32, Vec<u32>)> = vec![];
                    if mask & 1 != 0 {
                        expected_calls.push((0xFFFF_0000, interface, vec![1]));
                    }
                    if mask & 2 != 0 {
                        expected_calls.push((0xFFFF_0070, interface, vec![1]));
                    }
                    for (offset, ptr, count) in [(0xAC, first.as_ptr() as u32, 2), (0xBC, second.as_ptr() as u32, 1)] {
                        if populated {
                            for _ in 0..count {
                                expected_calls.push((0xFFFF_0000, interface, vec![1]));
                            }
                        }
                        expected_calls.push((
                            0x009A_1ADD,
                            this + offset,
                            vec![ptr, ptr + if populated { 12 } else { 0 }],
                        ));
                    }
                    if populated {
                        expected_calls.push((0xFFFF_0008, interface, vec![]));
                        expected_calls.push((0x0040_D4C0, RESULT_ECX, vec![p0]));
                        expected_calls.push((
                            if mask & 8 != 0 { 0xFFFF_0058 } else { 0xFFFF_0008 },
                            interface,
                            if mask & 8 != 0 { vec![0x9876_5432] } else { vec![] },
                        ));
                        expected_calls.push((0x0040_D4C0, RESULT_ECX, vec![p1]));
                    }
                    expected_calls.push((0x008F_FD4F, this + 0x198, vec![]));
                    for bit in [4, 8, 16] {
                        if mask & bit != 0 {
                            expected_calls.push((0xFFFF_0008, interface, vec![]));
                        }
                    }
                    let log = calls();
                    assert_eq!(targets(), expected_calls.iter().map(|c| c.0).collect::<Vec<_>>());
                    assert_eq!(
                        log.iter().map(|c| (c.this, c.arguments.clone())).collect::<Vec<_>>(),
                        expected_calls.iter().map(|c| (c.1, c.2.clone())).collect::<Vec<_>>()
                    );
                    for c in &log {
                        if c.this == interface {
                            assert_eq!(c.eax, vt.as_ptr() as u32);
                        }
                    }
                    STATE.with(|s| {
                        let s = s.borrow();
                        assert!(s.snapshots.iter().all(|snapshot| snapshot[0x18] == 0));
                    });
                    assert_eq!(object, expected);
                    assert_eq!(first, [interface, 0, interface]);
                    assert_eq!(second, [0, interface, 0]);
                    assert_eq!(nodes, nodes_before);
                    assert_eq!(header, header_before);
                }
            }
            start(None, [0; 8]);
            let mut object = [0xA5u8; 0x1B0];
            object[0x18] = 0;
            let before = object;
            let this = object.as_mut_ptr() as u32;
            assert_eq!(
                run(fc4p_00900357 as *const () as usize, this, &[]),
                [(ENTRY_EAX & !255) | 1, this, ENTRY_EDX]
            );
            assert_eq!(object, before);
            assert!(calls().is_empty());
        }

        #[test]
        fn document_vectors_reload_bounds_and_storage_after_deleting_callbacks() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                if address == 0xFFFF_0000 {
                    BATCH.with(|s| {
                        let v = s.borrow().values;
                        // SAFETY: the first callback switches the vector to a new two-element range.
                        unsafe {
                            put_word(v[0], 0xAC, v[1]);
                            put_word(v[0], 0xB0, v[1] + 8);
                        }
                    });
                }
                // SAFETY: these are the resource test's valid object ranges.
                unsafe {
                    resource_hook(r, address);
                }
            }
            let _fakes = install_many(&[0x009A_1ADD, 0x008F_FD4F]);
            let vt = [delete_value as *const () as usize];
            let mut a = [vt.as_ptr() as u32];
            let mut b = [vt.as_ptr() as u32];
            let mut initial = [a.as_mut_ptr() as u32];
            let mut replaced = [0, b.as_mut_ptr() as u32];
            let mut header = [0u32; 3];
            header[2] = header.as_mut_ptr() as u32;
            let mut object = [0u32; 0x1B0 / 4];
            let this = object.as_mut_ptr() as u32;
            object[0x18 / 4] = 1;
            object[0xAC / 4] = initial.as_mut_ptr() as u32;
            object[0xB0 / 4] = initial.as_mut_ptr() as u32 + 4;
            object[0x198 / 4] = header.as_mut_ptr() as u32;
            start(Some(hook), [this, replaced.as_mut_ptr() as u32, 0, 0, 0, 0, 0, 0]);
            run(fc4p_00900357 as *const () as usize, this, &[]);
            let log = calls();
            assert_eq!(
                targets(),
                [0xFFFF_0000, 0xFFFF_0000, 0x009A_1ADD, 0x009A_1ADD, 0x008F_FD4F]
            );
            assert_eq!([log[0].this, log[1].this], [a.as_ptr() as u32, b.as_ptr() as u32]);
            assert_eq!(
                log[2].arguments,
                [replaced.as_ptr() as u32, replaced.as_ptr() as u32 + 8]
            );
            assert_eq!(object[0xB0 / 4], replaced.as_ptr() as u32);
        }

        #[test]
        fn hanging_indent_destruction_captures_iteration_bounds_and_reloads_storage() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                let values = BATCH.with(|s| s.borrow().values);
                // SAFETY: callback replaces only the test object's vector fields.
                unsafe {
                    if address == 0xFFFF_0000 {
                        put_word(values[0], 0x20, values[1]);
                    } else if address == 0x009A_1ADD && values[2] != 0 {
                        put_word(r.ecx, 0, values[2]);
                    }
                    resource_hook(r, address);
                }
            }
            let _fakes = install_many(&[0x009A_1ADD, 0x0090_CF63]);
            for populated in [false, true] {
                let vt = [delete_value as *const () as usize];
                let mut value = [vt.as_ptr() as u32];
                let mut values = [value.as_mut_ptr() as u32, 0, value.as_mut_ptr() as u32];
                let begin = if populated { values.as_mut_ptr() as u32 } else { 0 };
                let mut object = [0xA5A5_A5A5u32; 12];
                let this = object.as_mut_ptr() as u32;
                object[7] = begin;
                object[8] = begin + if populated { 12 } else { 0 };
                let replacement = if populated { 0x7654_3210 } else { 0 };
                start(Some(hook), [this, begin + 4, replacement, 0, 0, 0, 0, 0]);
                STATE.with(|s| s.borrow_mut().watch = Some((this, 48)));
                let mut expected = object;
                expected[0] = 0x00AC_D504;
                expected[7] = replacement;
                expected[8] = begin;
                assert_eq!(
                    run(fc4p_009086b7 as *const () as usize, this, &[]),
                    [RESULT_EAX, if populated { replacement } else { RESULT_ECX }, RESULT_EDX]
                );
                let log = calls();
                assert_eq!(
                    targets(),
                    if populated {
                        vec![0xFFFF_0000, 0xFFFF_0000, 0x009A_1ADD, 0x0090_CF63]
                    } else {
                        vec![0x009A_1ADD]
                    }
                );
                let clear = &log[if populated { 2 } else { 0 }];
                assert_eq!(clear.this, this + 0x1C);
                assert_eq!(clear.arguments, [begin, if populated { begin + 4 } else { 0 }]);
                if populated {
                    assert_eq!(log[3].arguments, [replacement]);
                }
                STATE.with(|s| {
                    assert!(s
                        .borrow()
                        .snapshots
                        .iter()
                        .all(|v| v[..4] == 0x00AC_D5E8u32.to_le_bytes()))
                });
                assert_eq!(object, expected);
            }
        }

        #[test]
        fn seven_word_copy_calls_each_constructor_and_returns_the_output_end() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                // SAFETY: each supplied element has seven readable or writable words.
                unsafe {
                    for offset in (0..28).step_by(4) {
                        put_word(r.argument(0), offset, word(r.argument(1), offset));
                    }
                }
                results(r);
            }
            let _fake = install(0x009A_7610);
            for n in 0..=3 {
                start(Some(hook), [0; 8]);
                let mut source = [0u32; 21];
                for (i, value) in source.iter_mut().enumerate() {
                    *value = (i as u32).wrapping_mul(0x8765_4321);
                }
                let before = source;
                let begin = source.as_mut_ptr() as u32;
                let mut output = [0xA5A5_A5A5u32; 23];
                let destination = output.as_mut_ptr() as u32 + 4;
                assert_eq!(
                    invoke(
                        fc4p_0090990f as *const () as usize,
                        0x1234,
                        &[begin, begin + n * 28, destination],
                        ENTRY_EAX,
                        ENTRY_EDX,
                        true
                    ),
                    [
                        destination + n * 28,
                        if n == 0 { 0x1234 } else { begin + (n - 1) * 28 },
                        if n == 0 { ENTRY_EDX } else { RESULT_EDX }
                    ]
                );
                let log = calls();
                assert_eq!(log.len(), n as usize);
                for (i, call) in log.iter().enumerate() {
                    assert_eq!(
                        call,
                        &trace(
                            if i == 0 { 0x1234 } else { begin + (i as u32 - 1) * 28 },
                            if i == 0 { ENTRY_EAX } else { RESULT_EAX },
                            if i == 0 { ENTRY_EDX } else { RESULT_EDX },
                            &[destination + i as u32 * 28, begin + i as u32 * 28]
                        )
                    );
                }
                let count = n as usize * 7;
                assert_eq!(&output[1..1 + count], &source[..count]);
                assert!(output[1 + count..].iter().all(|&v| v == 0xA5A5_A5A5));
                assert_eq!(output[0], 0xA5A5_A5A5);
                assert_eq!(source, before);
            }
        }

        #[test]
        fn free_ring_activation_allocates_for_singletons_and_splices_both_active_shapes() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                let node = BATCH.with(|s| s.borrow().values[0]);
                results(r);
                r.eax = node;
            }
            for free_count in [1usize, 3] {
                for active_count in [0usize, 2] {
                    for flags in [0u32, 0xBFFF_FFFF, 0xC000_0000, u32::MAX] {
                        let mut nodes = [[0xA5A5_A5A5u32; 5]; 6];
                        let pointers = nodes.each_mut().map(|n| n.as_mut_ptr() as u32);
                        for i in 0..free_count {
                            nodes[i][0] = pointers[(i + 1) % free_count];
                            nodes[i][1] = pointers[(i + free_count - 1) % free_count];
                        }
                        for i in 0..2 {
                            nodes[4 + i][0] = pointers[5 - i];
                            nodes[4 + i][1] = pointers[5 - i];
                        }
                        for (i, node) in nodes.iter_mut().enumerate() {
                            node[2] = 0x1234_5678 + i as u32;
                        }
                        nodes[0][3] = flags;
                        let mut vt = [0usize; 3];
                        vt[2] = allocate_node as *const () as usize;
                        let mut object = [
                            vt.as_ptr() as u32,
                            pointers[0],
                            if active_count == 0 { 0 } else { pointers[4] },
                            0xABCD_EF01,
                        ];
                        let this = object.as_mut_ptr() as u32;
                        start(Some(hook), [pointers[3], 0, 0, 0, 0, 0, 0, 0]);
                        STATE.with(|s| s.borrow_mut().watch = Some((this, 16)));
                        let mut expected = nodes;
                        let remaining = if free_count == 1 {
                            expected[3][0] = pointers[3];
                            expected[3][1] = pointers[3];
                            pointers[3]
                        } else {
                            expected[1][1] = pointers[2];
                            expected[2][0] = pointers[1];
                            pointers[2]
                        };
                        expected[0][3] = flags.wrapping_add(0x4000_0000);
                        if active_count == 0 {
                            expected[0][0] = pointers[0];
                            expected[0][1] = pointers[0];
                        } else {
                            expected[0][0] = pointers[4];
                            expected[0][1] = pointers[5];
                            expected[4][1] = pointers[0];
                            expected[5][0] = pointers[0];
                        }
                        assert_eq!(
                            run(fc4p_0090cf9a as *const () as usize, this, &[]),
                            [
                                if free_count == 1 { nodes[3][2] } else { nodes[2][2] },
                                if active_count != 0 {
                                    pointers[5]
                                } else if free_count == 1 {
                                    pointers[3]
                                } else {
                                    pointers[1]
                                },
                                if free_count == 1 { pointers[0] } else { ENTRY_EDX }
                            ]
                        );
                        assert_eq!(nodes, expected);
                        assert_eq!(object, [vt.as_ptr() as u32, remaining, pointers[0], 0xABCD_EF01]);
                        if free_count == 1 {
                            assert_eq!(targets(), [0xFFFF_0100]);
                            assert_eq!(calls(), [trace(this, vt.as_ptr() as u32, ENTRY_EDX, &[])]);
                            STATE.with(|s| assert_eq!(s.borrow().snapshots[0][4..8], pointers[0].to_le_bytes()));
                        } else {
                            assert!(calls().is_empty());
                        }
                    }
                }
            }
        }

        #[test]
        fn free_ring_growth_reloads_the_owner_after_the_allocation_callback() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                let values = BATCH.with(|s| s.borrow().values);
                // SAFETY: the callback replaces the free-ring cursor with another live singleton.
                unsafe {
                    put_word(r.ecx, 4, values[1]);
                }
                results(r);
                r.eax = values[0];
            }
            let mut nodes = [[0xA5A5_A5A5u32; 5]; 3];
            let pointers = nodes.each_mut().map(|n| n.as_mut_ptr() as u32);
            for (node, &p) in nodes.iter_mut().zip(&pointers) {
                node[0] = p;
                node[1] = p;
            }
            let mut vt = [0usize; 3];
            vt[2] = allocate_node as *const () as usize;
            let mut object = [vt.as_ptr() as u32, pointers[0], 0];
            let this = object.as_mut_ptr() as u32;
            start(Some(hook), [pointers[1], pointers[2], 0, 0, 0, 0, 0, 0]);
            assert_eq!(
                run(fc4p_0090cf9a as *const () as usize, this, &[]),
                [0xA5A5_A5A5, pointers[0], pointers[2]]
            );
            assert_eq!(object, [vt.as_ptr() as u32, pointers[0], pointers[0]]);
            assert_eq!(&nodes[0][..2], &[pointers[0], pointers[0]]);
            assert_eq!(nodes[0][3], 0xE5A5_A5A5);
            assert_eq!(&nodes[1][..2], &[pointers[2], pointers[2]]);
            assert_eq!(&nodes[2][..2], &[pointers[1], pointers[1]]);
        }

        #[test]
        fn locked_pools_use_the_unsigned_boundary_and_return_the_correct_call_result() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                results(r);
                if address == 0x0093_0B6D {
                    r.eax = 0x1122_3344;
                    r.edx = 0x5566_7788;
                }
            }
            let _fakes = install_many(&[
                0x0093_0B35,
                0x0093_0B6D,
                0x0090_D6AB,
                0x0090_D494,
                0x0090_D7F2,
                0x0090_D5DB,
            ]);
            let mut globals = [0xA5u8; 3];
            let lock = globals.as_mut_ptr() as u32;
            let small_pool = lock + 1;
            let large_pool = lock + 2;
            let _data = [
                crate::testing::fake_data(0x00B6_25FC, lock as *mut u8),
                crate::testing::fake_data(0x00B6_25D4, small_pool as *mut u8),
                crate::testing::fake_data(0x00B6_25B0, large_pool as *mut u8),
            ];
            for size in [0, 1, 13, 14, 15, 0x7FFF_FFFF, 0x8000_0000, u32::MAX] {
                for free in [false, true] {
                    start(Some(hook), [0; 8]);
                    let pointer = 0x9876_5432;
                    let entry = if free { fc4p_0090d91c } else { fc4p_0090d8e0 };
                    let arguments = if free { vec![pointer, size] } else { vec![size] };
                    assert_eq!(
                        invoke(
                            entry as *const () as usize,
                            0x1234,
                            &arguments,
                            ENTRY_EAX,
                            ENTRY_EDX,
                            true
                        ),
                        [if free { 0x1122_3344 } else { RESULT_EAX }, RESULT_ECX, 0x5566_7788]
                    );
                    assert_eq!(
                        targets(),
                        [
                            0x0093_0B35,
                            match (free, size < 14) {
                                (false, true) => 0x0090_D6AB,
                                (false, false) => 0x0090_D494,
                                (true, true) => 0x0090_D7F2,
                                (true, false) => 0x0090_D5DB,
                            },
                            0x0093_0B6D
                        ]
                    );
                    assert_eq!(
                        calls(),
                        [
                            trace(lock, ENTRY_EAX, ENTRY_EDX, &[]),
                            trace(
                                if size < 14 { small_pool } else { large_pool },
                                if free { RESULT_EAX } else { size },
                                RESULT_EDX,
                                &arguments
                            ),
                            trace(lock, RESULT_EAX, RESULT_EDX, &[])
                        ]
                    );
                    assert_eq!(globals, [0xA5; 3]);
                }
            }
        }

        #[test]
        fn text_constructor_exposes_its_saved_word_and_clears_count_after_member_setup() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                // SAFETY: the tag points into the owner's saved register word.
                unsafe {
                    assert_eq!(r.eax, r.argument(0));
                    assert_eq!((r.eax as *const u8).read(), ((r.ecx - 4) >> 24) as u8);
                    (r.eax as *mut u8).write(0x12);
                    put_word(r.ecx, 4, 0x8765_4321);
                }
                results(r);
            }
            start(Some(hook), [0; 8]);
            let _fake = install(0x0093_F7EB);
            let mut object = [0xA5A5_A5A5u32; 4];
            let this = object.as_mut_ptr() as u32;
            STATE.with(|s| s.borrow_mut().watch = Some((this, 16)));
            assert_eq!(
                run(fc4p_0090dc0f as *const () as usize, this, &[]),
                [this, (this & 0x00FF_FFFF) | 0x1200_0000, RESULT_EDX]
            );
            assert_eq!(object, [0x00AC_E650, 0xA5A5_A5A5, 0, 0xA5A5_A5A5]);
            STATE.with(|s| {
                assert_eq!(
                    s.borrow().snapshots[0],
                    [0x00AC_E650u32, 0xA5A5_A5A5, 0xA5A5_A5A5, 0xA5A5_A5A5]
                        .into_iter()
                        .flat_map(u32::to_le_bytes)
                        .collect::<Vec<_>>()
                )
            });
            let log = calls();
            assert_eq!(
                log,
                [trace(this + 4, log[0].arguments[0], ENTRY_EDX, &log[0].arguments)]
            );
        }

        #[test]
        fn retained_text_append_reloads_the_argument_after_each_call() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                BATCH.with(|s| {
                    let mut s = s.borrow_mut();
                    // SAFETY: the retain call's ECX is the original writable argument slot.
                    unsafe {
                        if address == 0x004F_20C0 {
                            s.values[0] = r.ecx;
                            assert_eq!(word(r.ecx, 0), r.argument(0));
                            put_word(r.ecx, 0, s.values[1]);
                        } else if address == 0x0096_90F4 {
                            put_word(s.values[0], 0, s.values[2]);
                        }
                    }
                });
                results(r);
            }
            let _fakes = install_many(&[0x004F_20C0, 0x0096_90F4]);
            let mut vt = [0usize; 3];
            vt[2] = release_interface as *const () as usize;
            let mut interface = [vt.as_ptr() as u32];
            let pointer = interface.as_mut_ptr() as u32;
            for input in [0, pointer] {
                for retained in [0, pointer] {
                    for after_insert in [0, pointer] {
                        start(Some(hook), [0, retained, after_insert, 0, 0, 0, 0, 0]);
                        let mut object = [0xA5A5_A5A5u32; 4];
                        let this = object.as_mut_ptr() as u32;
                        assert_eq!(
                            run(fc4p_0090dc3d as *const () as usize, this, &[input]),
                            [
                                (RESULT_EAX & !255) | 1,
                                if after_insert == 0 { 0 } else { RESULT_ECX },
                                RESULT_EDX
                            ]
                        );
                        let log = calls();
                        assert_eq!(log[0], trace(log[0].this, ENTRY_EAX, ENTRY_EDX, &[input]));
                        assert_eq!(log[1], trace(this + 4, RESULT_EAX, RESULT_EDX, &[RESULT_EAX]));
                        if after_insert != 0 {
                            assert_eq!(log[2], trace(pointer, vt.as_ptr() as u32, RESULT_EDX, &[]));
                        }
                        assert_eq!(
                            targets(),
                            if after_insert == 0 {
                                vec![0x004F_20C0, 0x0096_90F4]
                            } else {
                                vec![0x004F_20C0, 0x0096_90F4, 0xFFFF_0008]
                            }
                        );
                        assert_eq!(object, [0xA5A5_A5A5; 4]);
                    }
                }
            }
        }

        #[test]
        fn error_mapping_checks_the_end_sentinel_and_passes_the_argument_slot() {
            unsafe fn hook(r: &mut Registers, _: u32) {
                // SAFETY: the search receives a writable argument word.
                unsafe {
                    assert_eq!(word(r.argument(2), 0), r.argument(3));
                    assert_eq!(r.eax, r.argument(2));
                    put_word(r.argument(2), 0, 0x1234_5678);
                }
                let result = BATCH.with(|s| s.borrow().values[0]);
                results(r);
                r.eax = result;
            }
            let _fake = install(0x0079_FA90);
            let mut entries = [0u32, 0x8000_0000, u32::MAX];
            let begin = entries.as_mut_ptr() as u32;
            let end = begin + 12;
            let _data = [
                crate::testing::fake_data(0x00A8_29B0, begin as *mut u8),
                crate::testing::fake_data(0x00A8_2A20, end as *mut u8),
            ];
            for i in 0..=3 {
                start(Some(hook), [begin + i * 4, 0, 0, 0, 0, 0, 0, 0]);
                assert_eq!(
                    run(fc4p_0090df7c as *const () as usize, 0x1234, &[0xDEAD_BEEF]),
                    [
                        if i == 3 { (-5i32) as u32 } else { entries[i as usize] },
                        RESULT_ECX,
                        RESULT_EDX
                    ]
                );
                let log = calls();
                assert_eq!(log.len(), 1);
                assert_eq!(
                    log[0],
                    trace(0x1234, log[0].eax, ENTRY_EDX, &[begin, end, log[0].eax, 0xDEAD_BEEF, 0])
                );
                assert_eq!(entries, [0, 0x8000_0000, u32::MAX]);
            }
        }

        #[test]
        fn string_insertion_checks_unsigned_limits_and_reloads_fields_after_errors() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                BATCH.with(|s| {
                    let values = s.borrow().values;
                    // SAFETY: these fields are inside the test's string object.
                    unsafe {
                        if address == 0x0040_7C40 {
                            put_word(values[0], 0, 0xFFFF_FFF0);
                            put_word(values[0], 4, 0x10);
                        } else if address == 0x0079_C6E0 {
                            put_word(values[0], 0, 0x8765_4321);
                        }
                    }
                });
                results(r);
            }
            let _fakes = install_many(&[0x0040_7C40, 0x0079_C6E0, 0x005C_2910]);
            for position in [0, 8, 9, 0x8000_0000, u32::MAX] {
                for count in [0, 1, 3, 0x7FFF_FFFF, u32::MAX - 2, u32::MAX - 1, u32::MAX] {
                    let mut object = [0x1234_5678u32, 0x1234_5680, 0xA5A5_A5A5];
                    let this = object.as_mut_ptr() as u32;
                    start(Some(hook), [this, 0, 0, 0, 0, 0, 0, 0]);
                    let bad_pos = position > 8;
                    let len = if bad_pos { 32 } else { 8 };
                    let bad_len = len > (u32::MAX - 1).wrapping_sub(count);
                    let begin = 0xFEDC_BA98u32;
                    assert_eq!(
                        run(fc4p_0090e8b6 as *const () as usize, this, &[position, begin, count]),
                        [this, RESULT_ECX, RESULT_EDX]
                    );
                    let mut expected = vec![];
                    if bad_pos {
                        expected.push(0x0040_7C40);
                    }
                    if bad_len {
                        expected.push(0x0079_C6E0);
                    }
                    expected.push(0x005C_2910);
                    assert_eq!(targets(), expected);
                    let log = calls();
                    let initial_base = if bad_pos { 0xFFFF_FFF0 } else { 0x1234_5678 };
                    let final_base: u32 = if bad_len { 0x8765_4321 } else { initial_base };
                    let had_error = bad_pos || bad_len;
                    let insert_position = final_base.wrapping_add(position);
                    assert_eq!(
                        log.last().unwrap(),
                        &trace(
                            this,
                            insert_position,
                            if had_error { RESULT_EDX } else { ENTRY_EDX },
                            &[insert_position, begin, begin.wrapping_add(count)]
                        )
                    );
                    if bad_pos {
                        assert_eq!(log[0], trace(this, 8, ENTRY_EDX, &[]));
                    }
                    if bad_len {
                        assert_eq!(
                            log[usize::from(bad_pos)],
                            trace(this, len, if bad_pos { RESULT_EDX } else { ENTRY_EDX }, &[])
                        );
                    }
                    assert_eq!(
                        object,
                        [final_base, if bad_pos { 0x10 } else { 0x1234_5680 }, 0xA5A5_A5A5]
                    );
                }
            }
        }

        #[test]
        fn word_fill_covers_every_alignment_zero_count_and_odd_tails_without_extra_writes() {
            for offset in 0..4 {
                for count in 0..=13 {
                    for value in [0u32, 0xFFFF, 0x1234_5678, u32::MAX] {
                        start(None, [0; 8]);
                        let mut output = [0xA5u8; 40];
                        let start = 4 + offset;
                        let pointer = output.as_mut_ptr().wrapping_add(start) as u32;
                        let mut expected = output;
                        for i in 0..count as usize {
                            expected[start + i * 2..start + i * 2 + 2].copy_from_slice(&(value as u16).to_le_bytes());
                        }
                        assert_eq!(
                            invoke(
                                fc4p_0090ffc8 as *const () as usize,
                                0x1234,
                                &[pointer, value, count],
                                ENTRY_EAX,
                                ENTRY_EDX,
                                true
                            ),
                            [
                                if count == 0 {
                                    ENTRY_EAX
                                } else {
                                    ((value & 0xFFFF) << 16) | (value & 0xFFFF)
                                },
                                0,
                                if count == 0 { ENTRY_EDX } else { pointer & 3 }
                            ]
                        );
                        assert_eq!(output, expected);
                        assert!(calls().is_empty());
                    }
                }
            }
        }

        #[test]
        fn guarded_release_skips_null_and_releases_the_guard_after_the_value() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                BATCH.with(|s| {
                    let mut s = s.borrow_mut();
                    // SAFETY: the constructor receives the two writable words of its local guard.
                    unsafe {
                        if address == 0x0048_C9F0 {
                            assert_eq!(word(r.ecx, 0), 0x8765_4321);
                            assert_eq!(word(r.ecx, 4), 0x8765_4321);
                            s.values[0] = r.ecx;
                            put_word(r.ecx, 4, s.values[1]);
                        } else if address == 0x0050_5190 {
                            put_word(s.values[0], 4, s.values[2]);
                        }
                    }
                });
                results(r);
            }
            let _fakes = install_many(&[0x0048_C9F0, 0x0050_5190]);
            let mut globals = [0xA5u8; 2];
            let global = globals.as_mut_ptr() as u32;
            let _data = [
                crate::testing::fake_data(0x00B6_2870, global as *mut u8),
                crate::testing::fake_data(0x00B6_27E0, (global + 1) as *mut u8),
            ];
            let mut vt = [0usize; 3];
            vt[2] = release_interface as *const () as usize;
            let mut first = [vt.as_ptr() as u32];
            let mut second = [vt.as_ptr() as u32];
            for value in [0, 1, 0x8000_0000, u32::MAX] {
                start(
                    Some(hook),
                    [0, first.as_mut_ptr() as u32, second.as_mut_ptr() as u32, 0, 0, 0, 0, 0],
                );
                let result = invoke(
                    fc4p_00913409 as *const () as usize,
                    0x8765_4321,
                    &[value],
                    ENTRY_EAX,
                    ENTRY_EDX,
                    true,
                );
                if value == 0 {
                    assert_eq!(result, [ENTRY_EAX, 0x8765_4321, ENTRY_EDX]);
                    assert!(calls().is_empty());
                } else {
                    assert_eq!(result, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                    let log = calls();
                    assert_eq!(
                        log,
                        [
                            trace(log[0].this, ENTRY_EAX, ENTRY_EDX, &[global]),
                            trace(global + 1, RESULT_EAX, RESULT_EDX, &[value]),
                            trace(second.as_ptr() as u32, vt.as_ptr() as u32, RESULT_EDX, &[])
                        ]
                    );
                    assert_eq!(targets(), [0x0048_C9F0, 0x0050_5190, 0xFFFF_0008]);
                }
                assert_eq!(globals, [0xA5; 2]);
            }
        }

        #[test]
        fn keyed_message_factory_checks_the_full_class_and_forwards_the_constructed_interface() {
            unsafe fn hook(r: &mut Registers, address: u32) {
                let mut eax = RESULT_EAX;
                BATCH.with(|s| {
                    let mut s = s.borrow_mut();
                    // SAFETY: the key is a writable pair supplied by the factory; both objects are test storage.
                    unsafe {
                        if address == 0x0044_A490 {
                            assert_eq!(word(r.ecx, 0), 0x7654_3210);
                            assert_eq!(word(r.ecx, 4), 0x7654_3210);
                            s.values[2] = r.ecx;
                            put_word(r.ecx, 0, 0x1234_5678);
                            put_word(r.ecx, 4, 0x8765_4321);
                        } else if address == 0x005E_55E0 {
                            eax = s.values[0];
                        } else if address == 0x0097_80DC {
                            eax = s.values[1];
                        }
                    }
                });
                results(r);
                r.eax = eax;
            }
            let _fakes = install_many(&[0x0044_A490, 0x005E_55E0, 0x0097_80DC]);
            let vt = [query_message as *const () as usize];
            let mut allocation = [0xA5A5_A5A5u32; 14];
            let allocated = allocation.as_mut_ptr() as u32;
            let mut constructed = [vt.as_ptr() as u32, 0x1234_5678];
            let interface = constructed.as_mut_ptr() as u32;
            for class in [0, 0x856D_DBAC, 0x856D_DBAD, 0x056D_DBAC, u32::MAX] {
                for key in [0, 0xFFFF, 0x1234_5678, u32::MAX] {
                    start(Some(hook), [allocated, interface, 0, 0, 0, 0, 0, 0]);
                    let result = run(
                        fc4p_0090b9e7 as *const () as usize,
                        0x7654_3210,
                        &[class, 0x8765_4321, 0xDEAD_BEEF, key, 0xCAFE_BABE],
                    );
                    if class == 0x856D_DBAC {
                        assert_eq!(result, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                        let log = calls();
                        assert_eq!(targets(), [0x0044_A490, 0x005E_55E0, 0x0097_80DC, 0xFFFF_0200]);
                        assert_eq!(
                            log,
                            [
                                trace(log[0].this, key & 0xFFFF, ENTRY_EDX, &[key & 0xFFFF]),
                                trace(RESULT_ECX, RESULT_EAX, RESULT_EDX, &[0x38]),
                                trace(allocated, allocated, RESULT_EDX, &[0x1234_5678, 0x8765_4321]),
                                trace(interface, interface, vt.as_ptr() as u32, &[0x8765_4321, 0xDEAD_BEEF])
                            ]
                        );
                    } else {
                        assert_eq!(result, [ENTRY_EAX & !255, 0x7654_3210, ENTRY_EDX]);
                        assert!(calls().is_empty());
                    }
                    assert_eq!(allocation, [0xA5A5_A5A5; 14]);
                    assert_eq!(constructed, [vt.as_ptr() as u32, 0x1234_5678]);
                }
            }
        }

        unsafe extern "thiscall" fn get_sample(frame: *mut Registers) -> *const crate::x87::F80 {
            // SAFETY: the fake supplies its entry registers; the sample array lives throughout the call.
            unsafe {
                observe_direct(frame, 0x0091_372E, 0);
            }
            let sample = BATCH.with(|s| {
                let mut s = s.borrow_mut();
                let sample = (s.values[0] as *const crate::x87::F80).wrapping_add(s.values[1] as usize);
                s.values[1] += 1;
                sample
            });
            // SAFETY: the frame belongs to this fake and is restored by its entry.
            unsafe {
                results(&mut *frame);
            }
            sample
        }

        #[unsafe(naked)]
        unsafe extern "C" fn sample_random() {
            core::arch::naked_asm!(
                "push edx", "push ecx", "push eax", "mov ecx, esp", "call {sample}",
                "fld tbyte ptr [eax]", "pop eax", "pop ecx", "pop edx", "ret",
                sample = sym get_sample,
            );
        }

        fn range_reference(samples: &[crate::x87::F80; 3], lower: f64, upper: f64, scale: f64) -> crate::x87::F80 {
            let mut saved = [0f64; 2];
            let mut answer = crate::x87::F80::ZERO;
            let operands = [
                samples.as_ptr() as usize,
                saved.as_mut_ptr() as usize,
                &lower as *const f64 as usize,
                &upper as *const f64 as usize,
                &scale as *const f64 as usize,
                &mut answer as *mut crate::x87::F80 as usize,
            ];
            // SAFETY: all operands span their full sizes; the calculation leaves the stack as it found it.
            unsafe {
                core::arch::asm!(
                    "mov eax, [edi]", "fld tbyte ptr [eax]", "mov eax, [edi + 4]", "fstp qword ptr [eax]",
                    "mov eax, [edi]", "fld tbyte ptr [eax + 10]", "mov eax, [edi + 4]", "fstp qword ptr [eax + 8]",
                    "mov eax, [edi]", "fld tbyte ptr [eax + 20]", "mov eax, [edi + 4]",
                    "fadd qword ptr [eax + 8]", "fadd qword ptr [eax]",
                    "mov eax, [edi + 12]", "fld qword ptr [eax]", "mov eax, [edi + 8]", "fsub qword ptr [eax]",
                    "fmulp st(1), st(0)", "mov eax, [edi + 16]", "fmul qword ptr [eax]",
                    "mov eax, [edi + 8]", "fadd qword ptr [eax]", "mov eax, [edi + 20]", "fstp tbyte ptr [eax]",
                    in("edi") &operands, out("eax") _, out("st(0)") _, out("st(1)") _,
                );
            }
            answer
        }

        fn invoke_float(arguments: &[u32]) -> ([u32; 3], crate::x87::F80) {
            #[repr(C)]
            struct FloatProbe {
                registers: Probe,
                result: crate::x87::F80,
            }
            // SAFETY: Probe contains only integers, byte arrays and raw pointers; each accepts zero.
            let mut call = FloatProbe {
                registers: unsafe { core::mem::zeroed() },
                result: crate::x87::F80::ZERO,
            };
            let r = &mut call.registers;
            r.target = fc4p_0091386b as *const () as usize;
            r.eax = ENTRY_EAX;
            r.ecx = 0x1234_5678;
            r.edx = ENTRY_EDX;
            r.args = arguments.as_ptr();
            r.count = arguments.len() as u32;
            r.x87_input = [0xEF, 0xCD, 0xAB, 0x89, 0x67, 0x45, 0x23, 0x81, 0xFF, 0x3F];
            assert_eq!(core::mem::offset_of!(FloatProbe, result), 88);
            // SAFETY: the probe supplies two doubles, receives ST0 and retains a separate preexisting value.
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [edi + 68]", "push edi", "call {probe}", "add esp, 4",
                    "fstp tbyte ptr [edi + 88]", "fstp tbyte ptr [edi + 78]",
                    in("edi") &mut call, probe = sym probe, clobber_abi("C"),
                    out("st(0)") _, out("st(1)") _, out("st(2)") _, out("st(3)") _,
                    out("st(4)") _, out("st(5)") _, out("st(6)") _, out("st(7)") _,
                );
            }
            let r = call.registers;
            assert_eq!(r.stack_delta, 0);
            assert_eq!((r.ebx, r.esi, r.ebp), (0x1357_9BDF, 0x2468_ACE0, 0x1122_3344));
            assert_eq!(r.flags & 0x400, 0);
            assert_eq!(r.cw_before, r.cw_after);
            assert_eq!(r.mxcsr_before, r.mxcsr_after);
            assert_eq!((r.sw_after >> 11) & 7, ((r.sw_before >> 11).wrapping_sub(1)) & 7);
            assert_eq!(r.x87_output, r.x87_input);
            ([r.eax, r.ecx, r.edx], call.result)
        }

        #[test]
        fn random_range_keeps_sample_order_double_spills_and_extended_result() {
            use crate::x87::{self, F80};
            let _fpu = crate::testing::GameFpu::new();
            let _fake = crate::testing::fake(0x0091_372E, sample_random as *const () as usize);
            let mut scale = f64::from_bits(0x3FD5_5555_5555_5555);
            let _data = crate::testing::fake_data(0x00AC_ECB0, (&mut scale as *mut f64).cast());
            let extended = F80::from_bytes([1, 0, 0, 0, 0, 0, 0, 0x80, 0xFF, 0x3F]);
            let huge = F80::from_bytes([0, 0, 0, 0, 0, 0, 0, 0x80, 0xFF, 0x43]);
            let samples = [
                [F80::from_f64(0.125), F80::from_f64(0.25), F80::from_f64(0.5)],
                [extended, F80::from_f64(-1.0), F80::ZERO],
                [F80::ZERO, F80::ZERO, extended],
                [huge, F80::ZERO, F80::ZERO],
                [F80::from_f64(-1.0e30), F80::from_f64(1.0e30), F80::from_f64(0.75)],
                [
                    F80::from_f64(f64::INFINITY),
                    F80::from_f64(f64::NEG_INFINITY),
                    F80::ZERO,
                ],
                [
                    F80::from_f64(f64::from_bits(0x7FF8_1234_5678_ABCD)),
                    F80::ZERO,
                    extended,
                ],
            ];
            for control in [0x007F, 0x037F, 0x077F, 0x0B7F, 0x0F7F] {
                x87::set_control_word(control);
                for samples in &samples {
                    for (lower, upper) in [
                        (0.0, 1.0),
                        (-17.25, 81.5),
                        (2.0, -3.0),
                        (-0.0, 0.0),
                        (-1.0e300, 1.0e300),
                        (f64::from_bits(1), f64::from_bits(4)),
                        (f64::from_bits(0xFFF0_1111_2222_3333), 1.0),
                        (0.0, f64::from_bits(0x7FF0_1111_2222_3333)),
                    ] {
                        start(None, [samples.as_ptr() as u32, 0, 0, 0, 0, 0, 0, 0]);
                        let expected = range_reference(samples, lower, upper, scale);
                        let mut args = [0u32; 4];
                        args[0] = lower.to_bits() as u32;
                        args[1] = (lower.to_bits() >> 32) as u32;
                        args[2] = upper.to_bits() as u32;
                        args[3] = (upper.to_bits() >> 32) as u32;
                        let top = x87::status_word() & 0x3800;
                        let (registers, result) = invoke_float(&args);
                        assert_eq!(
                            result, expected,
                            "control={control:04x} lower={lower:?} upper={upper:?}"
                        );
                        assert_eq!(registers, [RESULT_EAX, RESULT_ECX, RESULT_EDX]);
                        assert_eq!(x87::control_word(), control);
                        assert_eq!(x87::status_word() & 0x3800, top);
                        assert_eq!(
                            calls(),
                            [
                                trace(0x1234_5678, ENTRY_EAX, ENTRY_EDX, &[]),
                                trace(0x1234_5678, RESULT_EAX, RESULT_EDX, &[]),
                                trace(0x1234_5678, RESULT_EAX, RESULT_EDX, &[])
                            ]
                        );
                        assert_eq!(targets(), [0x0091_372E; 3]);
                    }
                }
            }
        }
    }
}
