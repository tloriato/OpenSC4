// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Construction and restoration of building development objects.

use crate::abi::{call_raw, target, vslot, Conv, RawCall};
use crate::port::Port;
use core::mem::MaybeUninit;
use core::ptr::{read_unaligned as read, write_unaligned as write};

game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> u64 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn list_node(this: *mut u8, count: u32, value: u32) -> u64 = "thiscall" 0x0041_3DD0; }
game_fn! { #[allow(dead_code)] fn reserve_plans(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0082_9BC0; }
game_fn! { #[allow(dead_code)] fn resize_words(this: *mut u8, count: u32, value: *mut u32) -> u64 = "thiscall" 0x0049_FC70; }
game_fn! { #[allow(dead_code)] fn insert_stage(this: *mut u8, out: *mut u32, side: u32, parent: u32, value: u32, force: u32) -> u64 = "thiscall" 0x0069_6630; }
game_fn! { #[allow(dead_code)] fn stage_entry(this: *mut u8, out: *mut u32, value: u32) -> u64 = "thiscall" 0x0069_6710; }
game_fn! { #[allow(dead_code)] fn stage_less(this: *mut u8, left: u32, right: u32) -> u64 = "thiscall" 0x0069_5CA0; }
game_fn! { #[allow(dead_code)] fn next_node(this: *mut u32) -> u64 = "thiscall" 0x005D_17C0; }
game_fn! { #[allow(dead_code)] fn previous_node(this: *mut u32) -> u64 = "thiscall" 0x0068_09E0; }
game_fn! { #[allow(dead_code)] fn read_reference(stream: *mut u8, iid: u32, out: *mut u32, immediate: u32) -> u64 = "C" 0x005D_9730; }
game_fn! { #[allow(dead_code)] fn developer_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0049_7A00; }
game_fn! { #[allow(dead_code)] fn stage_value(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x006C_0F30; }
game_fn! { #[allow(dead_code)] fn tried_entry(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x0069_AB40; }
game_fn! { #[allow(dead_code)] fn insert_tract(this: *mut u8, out: *mut u32, pair: *mut u32) -> u64 = "thiscall" 0x0069_6300; }
game_fn! { #[allow(dead_code)] fn find_plan(this: *mut u8, key: *mut u32) -> u64 = "thiscall" 0x004C_6920; }
game_fn! { #[allow(dead_code)] fn insert_plan(this: *mut u8, value: *mut u32) -> u64 = "thiscall" 0x0069_7470; }
game_fn! { #[allow(dead_code)] fn read_bool(stream: *mut u8, out: *mut u8) -> u64 = "C" 0x005D_9FA0; }
game_fn! { #[allow(dead_code)] fn get_system() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_interface(this: *mut u32) -> u64 = "thiscall" 0x0041_1870; }
game_data! { static list_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static tried_tree: u8 = 0x00B4_85E0; }

layout! {
    /// A developer's primary interfaces, rule limits, and nested collection fields.
    pub struct Developer {
        0x00 => primary / set_primary: u32,
        0x04 => messages / set_messages: u32,
        0x08 => serializable / set_serializable: u32,
        0x44 => tract_size / set_tract_size: u8,
        0x90 => residential_limit / set_residential_limit: u32,
        0x94 => commercial_limit / set_commercial_limit: u32,
        0x98 => industrial_limit / set_industrial_limit: u32,
        0x9C => enabled / set_enabled: u8,
        0xAD => minimum_x / set_minimum_x: u8,
        0xAE => minimum_z / set_minimum_z: u8,
        0xAF => maximum_size / set_maximum_size: u8,
        0xBC => growth_mode / set_growth_mode: u32,
    }
}

#[derive(Clone, Copy)]
struct Registers {
    eax: u32,
    edx: u32,
}
impl Registers {
    fn bits(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from(self.eax)
    }
    unsafe fn call(&mut self, conv: Conv, address: usize, this: u32, args: &[u32]) {
        let mut c = RawCall::new(conv, address, this, args);
        c.eax = self.eax;
        c.edx = self.edx;
        unsafe { call_raw(&mut c) };
        self.eax = c.eax;
        self.edx = c.edx;
    }
    unsafe fn game(&mut self, address: u32, this: u32, args: &[u32]) {
        unsafe { self.call(Conv::Thiscall, target(address), this, args) };
    }
    unsafe fn cdecl(&mut self, address: u32, this: u32, args: &[u32]) {
        unsafe { self.call(Conv::Cdecl, target(address), this, args) };
    }
    unsafe fn virtual_call(&mut self, object: *mut u8, slot: usize, args: &[u32], table_eax: bool) {
        let table = unsafe { read(object.cast::<u32>()) };
        if table_eax {
            self.eax = table;
        } else {
            self.edx = table;
        }
        unsafe { self.call(Conv::Thiscall, vslot(object, slot), object as u32, args) };
    }
}

macro_rules! entry {
    ($export:ident, $address:literal, $name:literal, $body:ident, $pop:literal) => {
        port! {
            /// Retains the caller's argument slots and both integer result registers.
            #[port(address = $address, owner = "building", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                core::arch::naked_asm!("push edx", "push eax", "lea edx, [esp + 12]",
                    "call {body}", concat!("ret ", $pop), body = sym $body);
            }
        }
    };
}

unsafe fn zero_words(this: *mut u8, offsets: &[usize]) {
    for &offset in offsets {
        unsafe { write(this.add(offset).cast::<u32>(), 0) };
    }
}

unsafe fn tree_header(this: *mut u8, offset: usize, r: &mut Registers, ecx: u32, parent_edx: bool) {
    unsafe {
        write(this.add(offset).cast::<u32>(), 0);
        r.cdecl(0x0090_CF54, ecx, &[0x18]);
        let node = r.eax as *mut u8;
        write(this.add(offset).cast::<*mut u8>(), node);
        write(this.add(offset + 4).cast::<u32>(), 0);
        write(node, 0);
        write(node.add(4).cast::<u32>(), 0);
        write(node.add(8).cast::<*mut u8>(), node);
        write(node.add(12).cast::<*mut u8>(), node);
        if parent_edx {
            r.edx = node as u32;
        }
    }
}

unsafe fn list_header(this: *mut u8, offset: usize, size: u32, r: &mut Registers, ecx: u32) {
    unsafe {
        write(this.add(offset).cast::<u32>(), 0);
        r.cdecl(0x0090_CF54, ecx, &[size]);
        let node = r.eax as *mut u8;
        write(node.cast::<*mut u8>(), node);
        write(node.add(4).cast::<*mut u8>(), node);
        write(this.add(offset).cast::<*mut u8>(), node);
    }
}

entry!(
    fc4p_0069a000,
    0x0069_A000,
    "cSC4BuildingDevelopmentSimulator::cSC4BuildingDevelopmentSimulator",
    construct_simulator,
    "0"
);
unsafe extern "fastcall" fn construct_simulator(this: *mut u8, _args: *mut u32, eax: u32, edx: u32) -> u64 {
    // SAFETY: the caller provides the simulator's 0xE4 bytes and the game owns every allocation.
    unsafe {
        let mut r = Registers { eax, edx };
        write(this.add(4).cast::<u32>(), 0x00A8_298C);
        write(this.add(8).cast::<u32>(), 0x00A8_1174);
        write(this.add(12).cast::<u32>(), 0x00A8_0784);
        write(this.cast::<u32>(), 0x00AB_0AF8);
        write(this.add(4).cast::<u32>(), 0x00AB_0AE8);
        write(this.add(8).cast::<u32>(), 0x00AB_0AD8);
        write(this.add(12).cast::<u32>(), 0x00AB_0AC0);
        write(this.add(0x10), 0);
        write(this.add(0x11), 0);
        zero_words(
            this,
            &[
                0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x40, 0x44, 0x48, 0x4C,
            ],
        );
        r.game(0x0041_3DD0, this.add(0x4C) as u32, &[1, 0]);
        let node = r.eax as *mut u8;
        write(node.cast::<*mut u8>(), node);
        write(node.add(4).cast::<*mut u8>(), node);
        write(this.add(0x4C).cast::<*mut u8>(), node);
        tree_header(this, 0x50, &mut r, this.add(0x4C) as u32, false);
        tree_header(this, 0x5C, &mut r, this.add(0x4C) as u32, false);
        let ecx = r.eax;
        zero_words(this, &[0x68, 0x6C, 0x70, 0x78]);
        list_header(this, 0x7C, 0x1C, &mut r, ecx);
        list_header(this, 0x80, 0x1C, &mut r, ecx);
        tree_header(this, 0x84, &mut r, ecx, true);
        tree_header(this, 0x90, &mut r, ecx, false);
        zero_words(this, &[0xA0, 0xA4, 0xA8, 0xAC]);
        r.game(0x0082_9BC0, this.add(0x9C) as u32, &[100]);
        tree_header(this, 0xB0, &mut r, this.add(0x9C) as u32, false);
        zero_words(this, &[0xBC, 0xD0, 0xD4, 0xD8]);
        for offset in 0xDC..=0xE1 {
            write(this.add(offset), 0);
        }
        write(this.add(0x74).cast::<u32>(), 0);
        r.eax = this as u32;
        r.bits()
    }
}

entry!(
    fc4p_006b7310,
    0x006B_7310,
    "ConstructBuildingDeveloper",
    construct_developer,
    "0"
);
unsafe extern "fastcall" fn construct_developer(this: *mut u8, _args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: the caller provides the developer's 0x13C bytes and its nested collections.
    unsafe {
        let d = Developer::from_ptr(this);
        d.set_messages(0x00A8_1174);
        d.set_serializable(0x00A8_0784);
        d.set_primary(0x00AB_1488);
        d.set_messages(0x00AB_1474);
        d.set_serializable(0x00AB_145C);
        write(this.add(0x0C), 0);
        zero_words(
            this,
            &[
                0x10, 0x14, 0x18, 0x1C, 0x20, 0x24, 0x28, 0x2C, 0x30, 0x34, 0x38, 0x3C, 0x40,
            ],
        );
        d.set_tract_size(0x40);
        for offset in 0x45..=0x47 {
            write(this.add(offset), 0);
        }
        zero_words(this, &[0x48, 0x4C, 0x50, 0x54, 0x58]);
        d.set_residential_limit(80);
        d.set_commercial_limit(80);
        d.set_industrial_limit(80);
        zero_words(
            this,
            &[
                0x5C, 0x60, 0x64, 0x68, 0x6C, 0x70, 0x74, 0x78, 0x7C, 0x80, 0x84, 0x88, 0x8C,
            ],
        );
        d.set_enabled(1);
        zero_words(this, &[0xA0, 0xA4, 0xA8]);
        write(this.add(0xAC), 0);
        d.set_minimum_x(3);
        d.set_minimum_z(3);
        d.set_maximum_size(6);
        let mut r = Registers { eax: 3, edx };
        tree_header(this, 0xB0, &mut r, this as u32, false);
        d.set_growth_mode(1);
        tree_header(this, 0xC0, &mut r, this as u32, false);
        let ecx = r.eax;
        list_header(this, 0xE4, 12, &mut r, ecx);
        zero_words(
            this,
            &[
                0xEC, 0xF0, 0xF4, 0xF8, 0xFC, 0x100, 0x104, 0x10C, 0x110, 0x114, 0x118, 0x11C, 0x120, 0x124, 0x128,
                0x12C, 0x130, 0x134, 0x138,
            ],
        );
        let mut zero = 0u32;
        r.edx = &mut zero as *mut u32 as u32;
        r.game(0x0049_FC70, this.add(0x50) as u32, &[4, r.edx]);
        zero_words(this, &[0xD4, 0xD8, 0xDC]);
        r.eax = this as u32;
        r.bits()
    }
}

entry!(
    fc4p_00696fd0,
    0x0069_6FD0,
    "InsertDeveloperStageWithHint",
    insert_hint,
    "12"
);
unsafe extern "fastcall" fn insert_hint(this: *mut u8, args: *mut u32, _eax: u32, edx: u32) -> u64 {
    // SAFETY: the tree, hint, value, output and caller-owned scratch slots are live game storage.
    unsafe {
        let header = read(this.cast::<u32>());
        let hint = *args.add(1);
        let value = *args.add(2);
        let out = *args;
        let mut r = Registers { eax: header, edx };
        if hint == word(header, 8) {
            r.eax = read(this.add(4).cast());
            if r.eax != 0 {
                r.eax = word(value, 0);
                let key = word(hint, 0x10);
                let less = if (key as i32) != r.eax as i32 {
                    (key as i32) > r.eax as i32
                } else {
                    byte(value, 4) < byte(hint, 0x14)
                };
                if less {
                    return insert_at(this, out, hint, hint, value, 0, &mut r);
                }
                let less = if (r.eax as i32) != key as i32 {
                    (r.eax as i32) > key as i32
                } else {
                    r.edx = (r.edx & !255) | u32::from(byte(hint, 0x14));
                    byte(hint, 0x14) < byte(value, 4)
                };
                if !less {
                    return existing(out, hint, r);
                }
                *args.add(2) = hint;
                r.game(0x005D_17C0, args.add(2) as u32, &[]);
                let next = *args.add(2);
                if next == read(this.cast::<u32>()) {
                    return insert_at(this, out, 0, hint, value, hint, &mut r);
                }
                r.eax = next.wrapping_add(0x10);
                r.game(0x0069_5CA0, this.add(8) as u32, &[value, r.eax]);
                if r.eax as u8 != 0 {
                    r.eax = word(hint, 12);
                    if r.eax == 0 {
                        return insert_at(this, out, 0, hint, value, hint, &mut r);
                    }
                    return insert_at(this, out, next, next, value, 0, &mut r);
                }
            }
        } else if hint == header {
            r.eax = word(header, 12);
            r.edx = word(r.eax, 0x10);
            let key = word(value, 0);
            let less = if (key as i32) != r.edx as i32 {
                (key as i32) > r.edx as i32
            } else {
                r.edx = (r.edx & !255) | u32::from(byte(r.eax, 0x14));
                byte(r.eax, 0x14) < byte(value, 4)
            };
            if less {
                return insert_at(this, out, 0, r.eax, value, hint, &mut r);
            }
            // The helper writes an iterator word followed by its insertion-success byte.
            let mut result = MaybeUninit::<[u32; 2]>::uninit();
            r.game(0x0069_6710, this as u32, &[result.as_mut_ptr() as u32, value]);
            write(out as *mut u32, word(r.eax, 0));
            r.eax = out;
            return r.bits();
        } else {
            *args.add(1) = hint;
            r.game(0x0068_09E0, args.add(1) as u32, &[]);
            r.eax = word(value, 0);
            let key = word(hint, 0x10);
            let less = if (key as i32) != r.eax as i32 {
                (key as i32) > r.eax as i32
            } else {
                r.edx = (r.edx & !255) | u32::from(byte(value, 4));
                byte(value, 4) < byte(hint, 0x14)
            };
            let flag = if less {
                let previous = *args.add(1);
                r.edx = word(previous, 0x10);
                write(args.add(2).cast::<u8>(), 1);
                let previous_less = if (r.eax as i32) != r.edx as i32 {
                    (r.eax as i32) > r.edx as i32
                } else {
                    r.eax = (r.eax & !255) | u32::from(byte(previous, 0x14));
                    byte(previous, 0x14) < byte(value, 4)
                };
                if previous_less {
                    r.eax = word(previous, 12);
                    if r.eax != 0 {
                        return insert_at(this, out, hint, hint, value, 0, &mut r);
                    }
                    return insert_at(this, out, 0, previous, value, previous, &mut r);
                }
                true
            } else {
                write(args.add(2).cast::<u8>(), 0);
                false
            };
            *args.add(1) = hint;
            r.game(0x005D_17C0, args.add(1) as u32, &[]);
            r.eax = (r.eax & !255) | u32::from(flag);
            if !flag {
                r.eax = hint.wrapping_add(0x10);
                r.game(0x0069_5CA0, this.add(8) as u32, &[r.eax, value]);
                if r.eax as u8 == 0 {
                    return existing(out, hint, r);
                }
                r.eax = *args.add(1);
                let next = r.eax;
                let accepted = if next == read(this.cast::<u32>()) {
                    true
                } else {
                    r.eax = next.wrapping_add(0x10);
                    r.game(0x0069_5CA0, this.add(8) as u32, &[value, r.eax]);
                    if r.eax as u8 != 0 {
                        r.eax = *args.add(1);
                        true
                    } else {
                        false
                    }
                };
                if accepted {
                    if word(hint, 12) == 0 {
                        return insert_at(this, out, 0, hint, value, hint, &mut r);
                    }
                    return insert_at(this, out, r.eax, r.eax, value, 0, &mut r);
                }
            }
        }
        // The result includes the insertion-success byte after the iterator word.
        let mut result = MaybeUninit::<[u32; 2]>::uninit();
        r.game(0x0069_6710, this as u32, &[result.as_mut_ptr() as u32, value]);
        r.edx = word(r.eax, 0);
        write(out as *mut u32, r.edx);
        r.eax = out;
        r.bits()
    }
}

unsafe fn word(base: u32, offset: usize) -> u32 {
    unsafe { read((base as *const u8).add(offset).cast()) }
}
unsafe fn byte(base: u32, offset: usize) -> u8 {
    unsafe { read((base as *const u8).add(offset)) }
}
unsafe fn existing(out: u32, node: u32, mut r: Registers) -> u64 {
    unsafe { write(out as *mut u32, node) };
    r.eax = out;
    r.bits()
}
unsafe fn insert_at(this: *mut u8, out: u32, side: u32, parent: u32, value: u32, force: u32, r: &mut Registers) -> u64 {
    unsafe { r.game(0x0069_6630, this as u32, &[out, side, parent, value, force]) };
    r.eax = out;
    r.bits()
}

port! {
    /// Restores the version-four references, collections, growth plans and cheat flags.
    /// The aligned scratch frame is retained across stream and reference calls.
    #[port(address = 0x0069_ACB0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::Read", naked)]
    pub unsafe extern "C" fn fc4p_0069acb0() {
        core::arch::naked_asm!(
            "push ebp", "mov ebp, esp", "and esp, -8", "sub esp, 0x54",
            "push ebx", "push esi", "push edi", "push edx", "lea edx, [esp + 4]",
            "lea eax, [ebp + 8]", "push eax", "call {body}",
            "pop edi", "pop esi", "pop ebx", "mov esp, ebp", "pop ebp", "ret 4",
            body = sym read_simulator,
        );
    }
}

unsafe fn read_field(r: &mut Registers, stream: *mut u8, slot: usize, frame: *mut u8, offset: usize, table_eax: bool) {
    unsafe {
        if !table_eax {
            r.eax = frame.add(offset) as u32;
        }
        r.virtual_call(stream, slot, &[frame.add(offset) as u32], table_eax);
    }
}

unsafe extern "fastcall" fn read_simulator(
    this: *mut u8,
    frame: *mut u8,
    stream_slot: *mut u32,
    entry_edx: u32,
) -> u64 {
    // SAFETY: the wrapper supplies the original aligned scratch area; the game supplies all interfaces.
    unsafe {
        let stream = *stream_slot as *mut u8;
        let mut r = Registers { eax: 0, edx: entry_edx };
        write(frame.add(0x20).cast::<*mut u8>(), this);
        r.virtual_call(stream, 0x48, &[], true);
        if r.eax as u8 != 0 {
            r.eax &= !255;
            return r.bits();
        }
        read_field(&mut r, stream, 0x1C, frame, 0x14, false);
        if read(frame.add(0x14).cast::<u16>()) != 4 {
            r.eax &= !255;
            return r.bits();
        }
        write(this.add(4), 1);
        r.cdecl(
            0x005D_9730,
            this.add(0x3C) as u32,
            &[stream as u32, 0xC9BD_939A, this.add(0x3C) as u32, 0],
        );
        r.cdecl(
            0x005D_9730,
            this.add(0x64) as u32,
            &[stream as u32, 0x0797_A4E8, this.add(0x64) as u32, 0],
        );
        read_field(&mut r, stream, 0x24, frame, 0x18, false);
        let mut i = 0u32;
        while i < read(frame.add(0x18).cast::<u32>()) {
            read_field(&mut r, stream, 0x24, frame, 0x28, false);
            write(frame.add(0x24).cast::<u32>(), 0);
            r.cdecl(
                0x005D_9730,
                frame.add(0x24) as u32,
                &[stream as u32, 0xE7CD_D662, frame.add(0x24) as u32, 0],
            );
            r.edx = frame.add(0x28) as u32;
            r.game(0x0049_7A00, this.add(0x50) as u32, &[r.edx]);
            write(r.eax as *mut u32, read(frame.add(0x24).cast()));
            r.eax = read(frame.add(0x18).cast());
            i = i.wrapping_add(1);
        }
        read_field(&mut r, stream, 0x24, frame, 0x18, false);
        write(frame.add(0x1C).cast::<u32>(), 0);
        while read(frame.add(0x1C).cast::<u32>()) < read(frame.add(0x18).cast::<u32>()) {
            write(frame.add(0x28).cast::<u32>(), 0);
            r.cdecl(
                0x005D_9730,
                frame.add(0x28) as u32,
                &[stream as u32, 0x26BB_767E, frame.add(0x28) as u32, 0],
            );
            r.edx = read(frame.add(0x20).cast());
            let header = word(r.edx, 0x40);
            let mut allocator = *list_allocator();
            if allocator.is_null() {
                write(frame.add(0x24).cast::<u32>(), 0);
                r.cdecl(0x0087_93EC, 0, &[]);
                if r.eax != 0 {
                    r.edx = word(r.eax, 0);
                    r.call(
                        Conv::Thiscall,
                        vslot(r.eax as *mut u8, 0x14),
                        r.eax,
                        &[0x3AE4_BEA3, 0x3AE4_BEAB, frame.add(0x24) as u32],
                    );
                }
                r.edx = read(frame.add(0x24).cast());
                *list_allocator() = r.edx as *mut u8;
                r.game(0x0041_1870, frame.add(0x24) as u32, &[]);
                allocator = *list_allocator();
            }
            r.virtual_call(allocator, 0x0C, &[12], true);
            let node = r.eax;
            if node.wrapping_add(8) != 0 {
                r.edx = read(frame.add(0x28).cast());
                write((node as *mut u8).add(8).cast::<u32>(), r.edx);
            }
            let previous = word(header, 4);
            write(node as *mut u32, header);
            write((node as *mut u8).add(4).cast::<u32>(), previous);
            write(previous as *mut u32, node);
            write((header as *mut u8).add(4).cast::<u32>(), node);
            let next = read(frame.add(0x1C).cast::<u32>()).wrapping_add(1);
            write(frame.add(0x1C).cast::<u32>(), next);
            r.eax = next;
        }
        read_field(&mut r, stream, 0x24, frame, 0x18, true);
        i = 0;
        while i < read(frame.add(0x18).cast::<u32>()) {
            read_field(&mut r, stream, 0x24, frame, 0x28, false);
            write(frame.add(0x24).cast::<u32>(), read(frame.add(0x28).cast()));
            read_field(&mut r, stream, 0x14, frame, 0x13, false);
            r.game(0x006C_0F30, this.add(0x44) as u32, &[frame.add(0x24) as u32]);
            let value = read(frame.add(0x13));
            r.edx = (r.edx & !255) | u32::from(value);
            write(r.eax as *mut u8, value);
            r.eax = read(frame.add(0x18).cast());
            i = i.wrapping_add(1);
        }
        read_field(&mut r, stream, 0x24, frame, 0x18, true);
        write(frame.add(0x1C).cast::<u32>(), 0);
        while read(frame.add(0x1C).cast::<u32>()) < read(frame.add(0x18).cast::<u32>()) {
            read_field(&mut r, stream, 0x24, frame, 0x24, false);
            r.game(0x0069_AB40, tried_tree() as u32, &[frame.add(0x24) as u32]);
            let tree = r.eax;
            read_field(&mut r, stream, 0x24, frame, 0x28, false);
            i = 0;
            while i < read(frame.add(0x28).cast::<u32>()) {
                read_field(&mut r, stream, 0x20, frame, 0x2C, false);
                read_field(&mut r, stream, 0x20, frame, 0x30, false);
                write(frame.add(0x34).cast::<u32>(), read(frame.add(0x2C).cast()));
                r.edx = read(frame.add(0x30).cast());
                r.eax = frame.add(0x34) as u32;
                write(frame.add(0x38).cast::<u32>(), r.edx);
                r.game(0x0069_6300, tree, &[frame.add(0x3C) as u32, r.eax]);
                r.eax = read(frame.add(0x28).cast());
                i = i.wrapping_add(1);
            }
            let next = read(frame.add(0x1C).cast::<u32>()).wrapping_add(1);
            write(frame.add(0x1C).cast::<u32>(), next);
            r.eax = next;
        }
        read_field(&mut r, stream, 0x24, frame, 0x18, false);
        write(frame.add(0x1C).cast::<u32>(), 0);
        while read(frame.add(0x1C).cast::<u32>()) < read(frame.add(0x18).cast::<u32>()) {
            write(frame.add(0x24).cast::<u32>(), 0);
            r.cdecl(
                0x005D_9730,
                frame.add(0x24) as u32,
                &[stream as u32, 0xE7CD_D662, frame.add(0x24) as u32, 0],
            );
            r.edx = read(frame.add(0x24).cast());
            write(frame.add(0x3C).cast::<u32>(), r.edx);
            read_field(&mut r, stream, 0x20, frame, 0x40, true);
            read_field(&mut r, stream, 0x20, frame, 0x44, false);
            read_field(&mut r, stream, 0x14, frame, 0x48, false);
            read_field(&mut r, stream, 0x24, frame, 0x30, false);
            r.game(0x004C_6920, this.add(0x90) as u32, &[frame.add(0x3C) as u32]);
            if r.eax == 0 {
                r.eax = read(frame.add(0x40).cast());
                r.edx = read(frame.add(0x3C).cast());
                write(frame.add(0x50).cast::<u32>(), r.eax);
                write(frame.add(0x4C).cast::<u32>(), r.edx);
                write(frame.add(0x54).cast::<u32>(), read(frame.add(0x44).cast()));
                r.edx = read(frame.add(0x48).cast());
                r.eax = frame.add(0x4C) as u32;
                write(frame.add(0x58).cast::<u32>(), r.edx);
                write(frame.add(0x5C).cast::<u32>(), 0);
                r.game(0x0069_7470, this.add(0x90) as u32, &[r.eax]);
                r.eax = r.eax.wrapping_add(0x10);
            } else {
                r.eax = r.eax.wrapping_add(0x14);
            }
            write(r.eax as *mut u32, read(frame.add(0x30).cast()));
            let developer = read(frame.add(0x24).cast::<*mut u8>());
            if !developer.is_null() {
                r.virtual_call(developer, 8, &[], false);
            }
            let next = read(frame.add(0x1C).cast::<u32>()).wrapping_add(1);
            write(frame.add(0x1C).cast::<u32>(), next);
            r.eax = next;
        }
        r.eax = this.add(0xD1) as u32;
        r.cdecl(0x005D_9FA0, stream as u32, &[stream as u32, r.eax]);
        r.cdecl(
            0x005D_9FA0,
            this.add(0xD2) as u32,
            &[stream as u32, this.add(0xD2) as u32],
        );
        r.cdecl(0x005D_9FA0, stream as u32, &[stream as u32, this.add(0xD3) as u32]);
        r.virtual_call(stream, 0x48, &[], false);
        r.eax = (r.eax & !255) | u32::from(r.eax as u8 == 0);
        r.bits()
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0069a000::PORT,
    fc4p_006b7310::PORT,
    fc4p_00696fd0::PORT,
    fc4p_0069acb0::PORT,
];
