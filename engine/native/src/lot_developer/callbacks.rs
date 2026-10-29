// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Building registration, light notifications and removal of lot texture occupants.

use super::maintenance::OccupantNode;
use super::queue::{QueueIterator, QueueRange};
use super::services::service_port;
use super::*;

game_fn! { #[allow(dead_code)] fn destroy_list(list: *mut *mut u8) -> u64 = "thiscall" 0x0041_3FE0; }
game_fn! { #[allow(dead_code)] fn remove_occupants(this: *mut u8, list: *mut *mut u8) -> u64 = "thiscall" 0x006C_62F0; }
game_fn! { #[allow(dead_code)] fn grow_queue(this: *mut u8, value: *const u32) -> u64 = "thiscall" 0x0062_66E0; }
game_fn! { #[allow(dead_code)] fn queue_at(this: *const QueueIterator, output: *mut QueueIterator, offset: i32) -> u64 = "thiscall" 0x006C_6860; }
game_fn! { #[allow(dead_code)] fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_service(service: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_data! { static node_allocator: *mut u8 = 0x00B2_07E4; }

unsafe fn direct(address: u32, this: *mut u8, args: &[u32], eax: u32, edx: u32) -> RawCall {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    unsafe {
        call_raw(&mut call);
    }
    call
}

service_port!(0x006C_8E10, fc4p_006c8e10, "LightBuildingAndLotProps", light, 8);

/// Lights the building, then sends the light message to flagged props in its lot's own rectangle.
/// The first caller argument becomes the list head; lazy allocation uses the second for its service.
unsafe extern "fastcall" fn light(this: *mut u8, stack: *mut u32, output: *mut u64) {
    unsafe {
        let developer = LotDeveloper::from_ptr(this);
        let building = stack.add(1).read_unaligned() as *mut u8;
        let mut call = virtual_call(
            building,
            0x44,
            &[stack.add(2).read_unaligned()],
            table(building),
            (output.read_unaligned() >> 32) as u32,
            false,
        );
        call = virtual_call(building, 0x0C, &[], call.eax, table(building), false);
        let occupant = call.eax as *mut u8;
        let rect = stack.cast::<u8>().sub(16).cast::<i32>();
        virtual_call(occupant, 0x2C, &[rect as u32], call.eax, table(occupant), false);
        let x = rect.read_unaligned().wrapping_add(rect.add(2).read_unaligned()) >> 1;
        let z = rect.add(1).read_unaligned().wrapping_add(rect.add(3).read_unaligned()) >> 1;
        let manager = developer.lot_manager();
        call = virtual_call(
            manager,
            0x18,
            &[x as u32, z as u32, 0],
            rect.read_unaligned() as u32,
            table(manager),
            false,
        );
        let lot = call.eax as *mut u8;
        if lot.is_null() {
            output.write_unaligned(result(call));
            return;
        }
        virtual_call(lot, 0x38, &[rect as u32], call.eax, table(lot), false);
        let min = stack.cast::<u8>().sub(24).cast::<i32>();
        let max = stack.cast::<u8>().sub(32).cast::<i32>();
        min.write_unaligned(rect.read_unaligned());
        min.add(1).write_unaligned(rect.add(1).read_unaligned());
        max.write_unaligned(rect.add(2).read_unaligned());
        max.add(1).write_unaligned(rect.add(3).read_unaligned());
        let head_slot = stack.add(1).cast::<*mut u8>();
        head_slot.write_unaligned(core::ptr::null_mut());
        let mut edx = max.add(1).read_unaligned() as u32;
        if node_allocator().read().is_null() {
            let service = stack.add(2).cast::<*mut u8>();
            service.write_unaligned(core::ptr::null_mut());
            call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0087_93EC), 0, &[]);
            call.eax = min.add(1).read_unaligned() as u32;
            call.edx = edx;
            call_raw(&mut call);
            let root = call.eax as *mut u8;
            if !root.is_null() {
                call = virtual_call(
                    root,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, service as u32],
                    call.eax,
                    table(root),
                    false,
                );
            }
            node_allocator().write(service.read_unaligned());
            edx = direct(
                0x0041_1870,
                service.cast(),
                &[],
                call.eax,
                service.read_unaligned() as u32,
            )
            .edx;
        }
        let allocator = node_allocator().read();
        call = virtual_call(allocator, 0x0C, &[12], table(allocator), edx, false);
        let head = call.eax as *mut u8;
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        head_slot.write_unaligned(head);
        let manager = developer.occupant_manager();
        call = virtual_call(
            manager,
            0xA4,
            &[head_slot as u32, min as u32, max as u32, 0xA823_821E, u32::MAX],
            head_slot as u32,
            table(manager),
            false,
        );
        if call.eax as u8 != 0 {
            let sentinel = head_slot.read_unaligned();
            call.eax = sentinel as u32;
            let mut current = OccupantNode::from_ptr(sentinel).next();
            while current != sentinel {
                let prop = OccupantNode::from_ptr(current).occupant();
                call = virtual_call(prop, 0x18, &[], call.eax, table(prop), false);
                let holder = call.eax as *mut u8;
                call = virtual_call(holder, 0x24, &[0x4A9F_188B], call.eax, table(holder), false);
                if call.eax != 0 {
                    let property = call.eax as *mut u8;
                    call = virtual_call(property, 0x14, &[], call.eax, table(property), false);
                    let variant = call.eax as *mut u8;
                    call = virtual_call(variant, 0xFC, &[], call.eax, table(variant), false);
                    if (call.eax as *const u8).read() != 0 {
                        call = virtual_call(prop, 0x3C, &[0x09D8_E0B6, 0], table(prop), call.edx, false);
                    }
                }
                call = virtual_call(prop, 8, &[], call.eax, table(prop), false);
                current = OccupantNode::from_ptr(current).next();
            }
        }
        call = direct(0x0041_3FE0, head_slot.cast(), &[], call.eax, call.edx);
        output.write_unaligned(result(call));
    }
}

service_port!(0x006C_8F80, fc4p_006c8f80, "RegisterNightBuilding", register, 4);

/// Retains the queried building for the night queue, applies the light flag, and restores the cursor
/// after callbacks. A failed query still releases a nonnull interface written into its output slot.
unsafe extern "fastcall" fn register(this: *mut u8, stack: *mut u32, output: *mut u64) {
    unsafe {
        let object = stack.add(1).read_unaligned() as *mut u8;
        let building_slot = stack.cast::<u8>().sub(0x24).cast::<*mut u8>();
        building_slot.write_unaligned(core::ptr::null_mut());
        let mut call = virtual_call(
            object,
            0,
            &[0x87DD_A3A9, building_slot as u32],
            table(object),
            building_slot as u32,
            false,
        );
        if call.eax as u8 != 0 {
            let building = building_slot.read_unaligned();
            virtual_call(building, 4, &[], table(building), call.edx, false);
            let developer = LotDeveloper::from_ptr(this);
            let offset = queue::distance(developer.night_begin(), developer.night_cursor());
            let queue = QueueRange::from_ptr(this.add(0x64));
            let end = queue.end();
            let value = building as u32;
            stack.add(1).write_unaligned(value);
            if end.current == end.last.wrapping_sub(1) {
                direct(
                    0x0062_66E0,
                    queue.as_ptr(),
                    &[stack.add(1) as u32],
                    stack.add(1) as u32,
                    end.last.wrapping_sub(1) as u32,
                );
            } else {
                if !end.current.is_null() {
                    end.current.write_unaligned(value);
                }
                // Only the current word of the end iterator changes on this path.
                this.add(0x74)
                    .cast::<*mut u32>()
                    .write_unaligned(end.current.wrapping_add(1));
            }
            let lit = u32::from(developer.lit());
            let building = building_slot.read_unaligned();
            virtual_call(building, 0x44, &[lit], lit, table(building), false);
            let begin = developer.night_begin();
            let begin_slot = stack.cast::<u8>().sub(0x20).cast::<QueueIterator>();
            begin_slot.write_unaligned(begin);
            let iterator_slot = stack.cast::<u8>().sub(0x10).cast::<QueueIterator>();
            call = direct(
                0x006C_6860,
                begin_slot.cast(),
                &[iterator_slot as u32, offset as u32],
                begin.last as u32,
                iterator_slot as u32,
            );
            let source = call.eax as *const u32;
            this.add(0x8C).cast::<u32>().write_unaligned(source.read_unaligned());
            call.edx = source.add(1).read_unaligned();
            this.add(0x90).cast::<u32>().write_unaligned(call.edx);
            this.add(0x94)
                .cast::<u32>()
                .write_unaligned(source.add(2).read_unaligned());
            call.edx = source.add(3).read_unaligned();
            this.add(0x98).cast::<u32>().write_unaligned(call.edx);
        }
        let building = building_slot.read_unaligned();
        if !building.is_null() {
            call = virtual_call(building, 8, &[], table(building), call.edx, false);
        }
        output.write_unaligned(result(call));
    }
}

port! {
    /// Removes base-texture occupants in a lot's rectangle. The list sentinel is allocated even for a
    /// null lot or missing occupant manager; only a successful query invokes the occupant remover.
    #[port(address = 0x006C_AAF0, owner = "lotdeveloper", name = "RemoveLotBaseTextures")]
    pub unsafe extern "fastcall" fn fc4p_006caaf0(this: *mut u8, entry_edx: u32, lot: *mut u8) -> u64 {
        unsafe {
            let (mut head, edx) = super::objects::new_list(entry_edx);
            let mut call = RawCall::new(Conv::Thiscall, 0, 0, &[]);
            call.eax = if lot.is_null() { head as u32 } else { LotDeveloper::from_ptr(this).occupant_manager() as u32 };
            call.edx = edx;
            if !lot.is_null() && call.eax != 0 {
                let mut rect = [0u32; 4];
                virtual_call(lot, 0x38, &[rect.as_mut_ptr() as u32], rect.as_mut_ptr() as u32, table(lot), false);
                let manager = LotDeveloper::from_ptr(this).occupant_manager();
                call = virtual_call(manager, 0xA4,
                    &[&mut head as *mut *mut u8 as u32, rect.as_ptr() as u32, rect.as_ptr().add(2) as u32, 0x497F_6D9D, u32::MAX],
                    &mut head as *mut *mut u8 as u32, table(manager), false);
                if call.eax as u8 != 0 {
                    call = direct(0x006C_62F0, this, &[&mut head as *mut *mut u8 as u32], call.eax, call.edx);
                }
            }
            result(direct(0x0041_3FE0, (&mut head as *mut *mut u8).cast(), &[], call.eax, call.edx))
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_006c8e10::PORT, fc4p_006c8f80::PORT, fc4p_006caaf0::PORT];
