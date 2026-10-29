// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Lot developer construction, tuning and queue storage.

use super::queue::QueueIterator;
use super::*;
use core::mem::MaybeUninit;

game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_data! { static message_base_table: u8 = 0x00A8_1174; }
game_data! { static serial_base_table: u8 = 0x00A8_0784; }
game_data! { static developer_table: u8 = 0x00AB_1B98; }
game_data! { static developer_message_table: u8 = 0x00AB_1B88; }
game_data! { static developer_serial_table: u8 = 0x00AB_1B70; }

game_fn! { fn allocate_object(size: u32) -> *mut u8 = "C" 0x005E_55E0; }
game_fn! { fn allocate_block(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { fn construct_developer(this: *mut u8) -> u64 = "thiscall" 0x006C_8650; }
game_fn! { fn construct_queue(this: *mut u8, count: u32) -> u64 = "thiscall" 0x0074_5980; }
game_fn! { fn construct_curve(this: *mut u8) -> u64 = "thiscall" 0x007D_0410; }
game_fn! { fn reserve_map(this: *mut u8, count: u32, front: u32) -> u64 = "thiscall" 0x006D_B4A0; }
game_fn! { fn fill_words(first: QueueIterator, last: QueueIterator, value: *const u32) -> u64 = "C" 0x006C_6530; }
game_fn! { fn property_u16(holder: *mut u8, id: u32, output: *mut u8) -> u64 = "C" 0x005F_D420; }
game_fn! { fn property_u32(holder: *mut u8, id: u32, output: *mut u8) -> u64 = "C" 0x005F_D480; }
game_fn! { fn property_u8(holder: *mut u8, id: u32, output: *mut u8) -> u64 = "C" 0x005F_D390; }
game_fn! { fn property_float(holder: *mut u8, id: u32, output: *mut u8) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn set_curve(this: *mut u8, property: *mut u8) -> u64 = "thiscall" 0x005F_4620; }
game_fn! { #[allow(dead_code)] fn query_interface(this: *mut u8, id: u32, output: *mut *mut u8) -> u64 = "thiscall" 0x006C_3B80; }

unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { p.add(offset).cast::<u32>().read_unaligned() }
}

unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { p.add(offset).cast::<u32>().write_unaligned(value) }
}

port! {
    /// Fill a range of 32-word blocks, reading the fill value again for every element.
    #[port(address = 0x006C_6530, owner = "lotdeveloper", name = "FillBuildingQueueWords")]
    pub unsafe extern "C" fn fc4p_006c6530(first: QueueIterator, last: QueueIterator, value: *const u32) -> u32 {
        unsafe {
            let mut current = first.current;
            let mut limit = first.last;
            let mut node = first.node;
            while current != last.current {
                current.write_unaligned(value.read_unaligned());
                current = current.wrapping_add(1);
                if current == limit {
                    current = node.add(1).read_unaligned();
                    node = node.add(1);
                    limit = current.wrapping_add(32);
                }
            }
            current as u32
        }
    }
}

port! {
    /// Forward a queue fill through its game entry point.
    #[port(address = 0x006C_6A90, owner = "lotdeveloper", name = "FillBuildingQueue")]
    pub unsafe extern "C" fn fc4p_006c6a90(first: QueueIterator, last: QueueIterator, value: *const u32) -> u64 {
        unsafe { fill_words(first, last, value) }
    }
}

port! {
    /// Reserve front map slots and allocate each additional 32-word block.
    #[port(address = 0x006C_6FD0, owner = "lotdeveloper", name = "ReserveBuildingQueueFrontBlocks")]
    pub unsafe extern "thiscall" fn fc4p_006c6fd0(this: *mut u8, count: u32) -> u32 {
        unsafe {
            let blocks = count.wrapping_add(31) >> 5;
            let available = word(this, 0x0C).wrapping_sub(word(this, 0x20)) as i32 >> 2;
            let mut eax = available as u32;
            if blocks > available as u32 {
                eax = reserve_map(this, blocks, 1) as u32;
            }
            for i in 0..blocks {
                let block = allocate_block(0x80);
                let slot = word(this, 0x0C).wrapping_sub(i.wrapping_add(1).wrapping_mul(4));
                (slot as *mut u32).write_unaligned(block as u32);
                eax = block as u32;
            }
            eax
        }
    }
}

port! {
    /// Query the primary interface through the message-target interface.
    #[port(address = 0x006C_7340, owner = "lotdeveloper", name = "MessageTargetQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006c7340() {
        core::arch::naked_asm!("sub ecx, 4", "mov eax, 0x006c3b80", "jmp eax");
    }
}

port! {
    /// Query the primary interface through the serializable interface.
    #[port(address = 0x006C_7350, owner = "lotdeveloper", name = "SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006c7350() {
        core::arch::naked_asm!("sub ecx, 8", "mov eax, 0x006c3b80", "jmp eax");
    }
}

port! {
    /// Construct the developer's interfaces, services, queue, tuning and rule tree. Padding is retained.
    #[port(address = 0x006C_8650, owner = "lotdeveloper", name = "ConstructLotDeveloper")]
    pub unsafe extern "thiscall" fn fc4p_006c8650(this: *mut u8) -> u64 {
        unsafe {
            this.add(4).cast::<u32>().write_volatile(message_base_table() as u32);
            this.add(8).cast::<u32>().write_volatile(serial_base_table() as u32);
            put(this, 0, developer_table() as u32);
            put(this, 4, developer_message_table() as u32);
            put(this, 8, developer_serial_table() as u32);
            this.add(0x0C).write(0);
            for offset in (0x10..=0x58).step_by(4) { put(this, offset, 0); }
            this.add(0x5C).write(0);
            this.add(0x5D).write(1);
            this.add(0x5E).write(1);
            this.add(0x5F).write(0);
            for offset in (0x60..=0x88).step_by(4) { put(this, offset, 0); }
            construct_queue(this.add(0x64), 0);
            for offset in (0..16).step_by(4) { put(this, 0x8C + offset, word(this, 0x64 + offset)); }
            put(this, 0x9C, 0);
            put(this, 0xA0, 8);
            put(this, 0xA4, 200);
            this.add(0xA8).write(0);
            for offset in (0xAC..=0xBC).step_by(4) { put(this, offset, 0); }
            this.add(0xC0).cast::<u16>().write_unaligned(90);
            put(this, 0xC4, 0x3F19_999A);
            put(this, 0xC8, 0x3ECC_CCCD);
            put(this, 0xCC, 0x3E4C_CCCD);
            put(this, 0xD0, 0x3F80_0000);
            put(this, 0xD4, 0x3F80_0000);
            put(this, 0xD8, 0x890B_7312);
            put(this, 0xDC, 0x890B_7312);
            this.add(0xE0).write(1);
            this.add(0xE1).write(1);
            put(this, 0xE4, 0);
            put(this, 0xE8, 0);
            construct_curve(this.add(0xEC));
            put(this, 0xFC, 0);
            let header = allocate_block(0x1C);
            put(this, 0xFC, header as u32);
            put(this, 0x100, 0);
            header.write(0);
            put(header, 4, 0);
            let header = word(this, 0xFC) as *mut u8;
            put(header, 8, header as u32);
            let header = word(this, 0xFC) as *mut u8;
            put(header, 12, header as u32);
            put(this, 0xAC, 3);
            put(this, 0xB0, 0);
            put(this, 0xBC, 0);
            (u64::from(header as u32) << 32) | u64::from(this as u32)
        }
    }
}

port! {
    /// Allocate and construct a lot developer, returning null if allocation fails.
    #[port(address = 0x006C_8DF0, owner = "lotdeveloper", name = "CreateLotDeveloper")]
    pub unsafe extern "C" fn fc4p_006c8df0() -> *mut u8 {
        unsafe {
            let object = allocate_object(0x108);
            if object.is_null() { object } else { construct_developer(object) as u32 as *mut u8 }
        }
    }
}

port! {
    /// Read the lot-development exemplar, set condition defaults, and update the response curve.
    #[port(address = 0x006C_4A20, owner = "lotdeveloper", name = "cSC4LotDeveloper::ReadSimulatorTunableValues")]
    pub unsafe extern "thiscall" fn fc4p_006c4a20(this: *mut u8) -> u32 {
        unsafe {
            let key = [0x6534_284A, 0xE7E2_C2DB, 0xE8DA_7677];
            let mut resource = MaybeUninit::<*mut u8>::uninit();
            let eax = vcall!(resource_manager().read(), 0x0C => fn(*const u32, u32, *mut *mut u8, u32, u32) -> u32,
                key.as_ptr(), 0xA521_60F5, resource.as_mut_ptr(), 0, 0);
            if eax as u8 == 0 { return eax; }
            let resource = resource.assume_init();
            let holder = vcall!(resource, 0x1C => fn() -> *mut u8);
            property_u16(holder, 0x47E2_C400, this.add(0xC0));
            put(this, 0xCC, 0x3E4C_CCCD);
            put(this, 0xC8, 0x3ECC_CCCD);
            put(this, 0xC4, 0x3F19_999A);
            let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0x47E2_C401);
            if !property.is_null() {
                let variant = vcall!(property, 0x14 => fn() -> *mut u8);
                let values = vcall!(variant, 0x1BC => fn() -> *mut u8);
                put(this, 0xCC, word(values, 0));
                put(this, 0xC8, word(values, 4));
                put(this, 0xC4, word(values, 8));
            }
            property_float(holder, 0xE900_EA24, this.add(0xD0));
            property_float(holder, 0xE900_EA25, this.add(0xD4));
            property_u32(holder, 0xE900_EA26, this.add(0xD8));
            property_u32(holder, 0x6916_516B, this.add(0xDC));
            property_u8(holder, 0xE900_EA27, this.add(0xE0));
            property_u8(holder, 0xE900_EA28, this.add(0xE1));
            property_float(holder, 0xE900_EA29, this.add(0xE4));
            property_float(holder, 0xE900_EA2A, this.add(0xE8));
            property_u32(holder, 0xE900_EA2B, this.add(0xA4));
            property_u32(holder, 0xE900_EA2C, this.add(0xA0));
            let property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, 0x2A2D_7824);
            if !property.is_null() { set_curve(this.add(0xEC), property); }
            vcall!(resource, 8 => fn() -> u32)
        }
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006c4a20::PORT,
    fc4p_006c6530::PORT,
    fc4p_006c6a90::PORT,
    fc4p_006c6fd0::PORT,
    fc4p_006c7340::PORT,
    fc4p_006c7350::PORT,
    fc4p_006c8650::PORT,
    fc4p_006c8df0::PORT,
];
