// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Connection occupants assembled in configuration order and submitted as one circular list.

use super::maintenance::OccupantNode;
use super::textures::TextureObject;
use super::*;

game_data! { static node_allocator: *mut u8 = 0x00B2_07E4; }
game_data! { static initial_position: u32 = 0x00B4_9AEC; }
game_data! { static cell_scale: f32 = 0x00AB_1B04; }
game_data! { static half: f32 = 0x00A8_4D2C; }
game_data! { static rounding_bias: f32 = 0x00AB_1B08; }

game_fn! { #[allow(dead_code)] fn framework() -> u64 = "C" 0x0087_93EC; }
game_fn! { #[allow(dead_code)] fn release_service(this: *mut *mut u8) -> u64 = "thiscall" 0x0041_1870; }
game_fn! { #[allow(dead_code)] fn network_service() -> u64 = "C" 0x0060_2680; }
game_fn! { #[allow(dead_code)] fn clear_nodes(this: *mut *mut u8) -> u64 = "thiscall" 0x0041_3E60; }

unsafe fn direct(address: u32, conv: Conv, this: u32, args: &[u32], previous: RawCall) -> RawCall {
    let mut call = RawCall::new(conv, crate::abi::target(address), this, args);
    call.eax = previous.eax;
    call.edx = previous.edx;
    unsafe { call_raw(&mut call) };
    call
}

unsafe fn allocator(mut call: RawCall, temporary: *mut *mut u8) -> (*mut u8, RawCall) {
    unsafe {
        if node_allocator().read().is_null() {
            temporary.write_unaligned(core::ptr::null_mut());
            call = direct(0x0087_93EC, Conv::Cdecl, 0, &[], call);
            let root = call.eax as *mut u8;
            if !root.is_null() {
                call = virtual_call(
                    root,
                    0x14,
                    &[0x3AE4_BEA3, 0x3AE4_BEAB, temporary as u32],
                    call.eax,
                    table(root),
                    false,
                );
            }
            call.edx = temporary.read_unaligned() as u32;
            node_allocator().write(call.edx as *mut u8);
            call = direct(0x0041_1870, Conv::Thiscall, temporary as u32, &[], call);
        }
        (node_allocator().read(), call)
    }
}

/// The cell conversion uses a stored rounding bias and a bit adjustment, then compares the original
/// scaled value with the next integer. The shifts in connection masks also wrap at 32 bits.
fn cell(position: f32) -> i32 {
    unsafe {
        let scaled = terrain::multiply_float(F80::from_f32(position), cell_scale().read());
        let mut biased = 0f32;
        core::arch::asm!(
            "fld tbyte ptr [{scaled}]", "fld dword ptr [{half}]", "fsubr st(0), st(1)",
            "fadd dword ptr [{bias}]", "fstp dword ptr [{output}]", "fstp st(0)",
            scaled = in(reg) &scaled, half = in(reg) half(), bias = in(reg) rounding_bias(),
            output = in(reg) &mut biased, out("st(0)") _, out("st(1)") _,
        );
        let base = biased.to_bits().wrapping_add(0xB4C0_0000) as i32;
        let next = base.wrapping_add(1);
        let status: u16;
        core::arch::asm!(
            "fld tbyte ptr [{scaled}]", "fild dword ptr [{next}]", "fxch", "fucompp", "fnstsw ax",
            scaled = in(reg) &scaled, next = in(reg) &next, out("ax") status,
            out("st(0)") _, out("st(1)") _,
        );
        if ((status >> 8) & 0x44).count_ones() & 1 != 0 {
            next
        } else {
            base
        }
    }
}

port! {
    /// Retains the caller's argument slots and the original temporary list and output locations.
    /// The third word is unused; all twelve argument bytes are popped.
    #[port(address = 0x006C_9C80, owner = "lotdeveloper", name = "cSC4LotDeveloper::AddNetworkConnectionNodes", naked)]
    pub unsafe extern "C" fn fc4p_006c9c80() {
        core::arch::naked_asm!(
            "sub esp, 128", "mov [esp], eax", "mov [esp + 4], edx",
            "lea edx, [esp + 128]", "push esp", "call {}",
            "add esp, 128", "ret 12", sym add_connections,
        );
    }
}

unsafe extern "fastcall" fn add_connections(this: *mut u8, stack: *mut u8, registers: *const [u32; 2]) -> u64 {
    unsafe {
        let args = stack.add(4).cast::<u32>();
        let config = args.add(1).read_unaligned() as *mut u8;
        let mut call = virtual_call(config, 0x5C, &[], table(config), (*registers)[1], false);
        let objects = call.eax as *mut u32;
        let list = stack.sub(0x40).cast::<*mut u8>();
        let temporary = stack.sub(0x3C).cast::<*mut u8>();
        list.write_unaligned(core::ptr::null_mut());
        let (allocation, returned) = allocator(call, temporary);
        call = virtual_call(allocation, 0x0C, &[12], table(allocation), returned.edx, false);
        let head = call.eax as *mut u8;
        OccupantNode::from_ptr(head).set_next(head);
        OccupantNode::from_ptr(head).set_previous(head);
        list.write_unaligned(head);
        if !objects.is_null() {
            let bytes = objects.add(1).read_unaligned().wrapping_sub(objects.read_unaligned()) as i32;
            let high = ((i64::from(bytes) * 0x4EC4_EC4F) >> 32) as i32 >> 4;
            let mut remaining = high.wrapping_add((high as u32 >> 31) as i32);
            call.eax = remaining as u32;
            call.edx = high as u32;
            let mut offset = 0u32;
            while remaining > 0 {
                let record = objects.read_unaligned().wrapping_add(offset) as *mut u8;
                let object = TextureObject::from_ptr(record);
                if object.kind() == 7 {
                    let position = stack.sub(12).cast::<u32>();
                    let initial = initial_position().read();
                    position.add(2).write_unaligned(initial);
                    position.write_unaligned(initial);
                    position.add(1).write_unaligned(initial);
                    let facing = stack.sub(0x2C).cast::<u32>();
                    call = virtual_call(
                        this,
                        0x9C,
                        &[
                            args.read_unaligned(),
                            config as u32,
                            record as u32,
                            position as u32,
                            facing as u32,
                        ],
                        args.read_unaligned(),
                        table(this),
                        false,
                    );
                    let x = cell(f32::from_bits(position.read_unaligned()));
                    let z = cell(f32::from_bits(position.add(2).read_unaligned()));
                    let ids = object.candidates();
                    let count = (object.candidates_end() as u32).wrapping_sub(ids as u32) as i32 >> 2;
                    if count as u32 >= 3 {
                        let kind = ids.read_unaligned();
                        let mut b = ids.add(1).read_unaligned();
                        let mut c = ids.add(2).read_unaligned();
                        let orientation = facing.read_unaligned();
                        if orientation != 0 {
                            let shift = orientation.wrapping_mul(8);
                            let reverse = 32u32.wrapping_sub(shift);
                            b = b.wrapping_shl(shift).wrapping_add(b.wrapping_shr(reverse));
                            c = c.wrapping_shl(shift).wrapping_add(c.wrapping_shr(reverse));
                        }
                        let ids = object.candidates();
                        let count = (object.candidates_end() as u32).wrapping_sub(ids as u32) as i32 >> 2;
                        let d = if count as u32 >= 4 {
                            ids.add(3).read_unaligned()
                        } else {
                            0
                        };
                        call.eax = count as u32;
                        call = direct(0x0060_2680, Conv::Cdecl, 0, &[], call);
                        let service = call.eax as *mut u8;
                        let coords = stack.sub(0x14).cast::<u32>();
                        coords.write_unaligned(x as u32);
                        coords.add(1).write_unaligned(z as u32);
                        let output = stack.sub(0x20).cast::<u32>();
                        call = virtual_call(
                            service,
                            0x50,
                            &[output as u32, kind, coords as u32, b, c, d, orientation],
                            call.eax,
                            table(service),
                            false,
                        );
                        let sentinel = list.read_unaligned();
                        let (allocation, returned) = allocator(call, temporary);
                        call = virtual_call(allocation, 0x0C, &[12], table(allocation), returned.edx, false);
                        let node = call.eax as *mut u8;
                        let value = node.wrapping_add(8);
                        if !value.is_null() {
                            call.edx = output.read_unaligned();
                            value.cast::<u32>().write_unaligned(call.edx);
                        }
                        let previous = OccupantNode::from_ptr(sentinel).previous();
                        OccupantNode::from_ptr(node).set_next(sentinel);
                        OccupantNode::from_ptr(node).set_previous(previous);
                        OccupantNode::from_ptr(previous).set_next(node);
                        OccupantNode::from_ptr(sentinel).set_previous(node);
                    }
                }
                offset = offset.wrapping_add(52);
                remaining = remaining.wrapping_sub(1);
                call.eax = remaining as u32;
            }
        }
        call = direct(0x0060_2680, Conv::Cdecl, 0, &[], call);
        let service = call.eax as *mut u8;
        virtual_call(
            service,
            0x54,
            &[args.read_unaligned(), list as u32],
            call.eax,
            table(service),
            false,
        );
        let manager = LotDeveloper::from_ptr(this).occupant_manager();
        call = virtual_call(manager, 0xC0, &[list as u32], list as u32, table(manager), false);
        call = direct(0x0041_3E60, Conv::Thiscall, list as u32, &[], call);
        let sentinel = list.read_unaligned();
        let (allocation, returned) = allocator(call, args.add(1).cast());
        call = virtual_call(
            allocation,
            0x10,
            &[sentinel as u32],
            table(allocation),
            returned.edx,
            false,
        );
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006c9c80::PORT];
