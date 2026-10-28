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

//! Shared helpers, group 03: small functions that several classes use. Each function is independent; its game functions and globals are declared in this file.

use crate::abi::{call_raw, vslot, Conv, RawCall};
use crate::port::Port;
use core::ptr::{read_unaligned, write_unaligned};

// Some helpers leave a callee's EDX visible, including when that callee changes only a byte.
fn registers(eax: u32, edx: u32) -> u64 {
    (u64::from(edx) << 32) | u64::from(eax)
}

unsafe fn word(object: *mut u8, offset: usize) -> u32 {
    // SAFETY: the enclosing port supplies a readable original field.
    unsafe { read_unaligned(object.wrapping_add(offset).cast()) }
}

unsafe fn store(object: *mut u8, offset: usize, value: u32) {
    // SAFETY: the enclosing port supplies a writable original field.
    unsafe { write_unaligned(object.wrapping_add(offset).cast(), value) }
}

unsafe fn invoke(conv: Conv, target: usize, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> u64 {
    let mut call = RawCall::new(conv, target, this as u32, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: the enclosing port supplies the original target, register inputs and argument words.
    unsafe { call_raw(&mut call) };
    registers(call.eax, call.edx)
}

unsafe fn virtual_call(this: *mut u8, slot: usize, eax: u32, edx: u32, args: &[u32]) -> u64 {
    // SAFETY: the enclosing port supplies a live object and its original virtual signature.
    unsafe { invoke(Conv::Thiscall, vslot(this, slot), this, eax, edx, args) }
}

game_fn! { #[allow(dead_code)] fn destroy_bus(this: *mut u8) = "thiscall" 0x0050_4E70; }
game_fn! { #[allow(dead_code)] fn destroy_ambulance(this: *mut u8) = "thiscall" 0x0050_4EF0; }
game_fn! { #[allow(dead_code)] fn destroy_train(this: *mut u8) = "thiscall" 0x0053_CB80; }
game_fn! { fn free_sized_bus(object: *mut u8, size: u32) = "C" 0x0055_CA60; }
game_fn! { #[allow(dead_code)] fn heap_free(object: *mut u8) -> u64 = "C" 0x005E_5620; }
game_fn! { #[allow(dead_code)] fn array_free(object: *mut u8) -> u64 = "C" 0x0090_CF63; }
game_fn! { #[allow(dead_code)] fn destroy_controller_list(this: *mut u8) -> u64 = "thiscall" 0x0056_50A0; }
game_fn! { #[allow(dead_code)] fn destroy_controller(this: *mut u8) -> u64 = "thiscall" 0x0051_7F80; }
game_fn! { fn write_vehicle(this: *mut u8, stream: *mut u8) -> u32 = "thiscall" 0x0055_F640; }
game_fn! { fn remove_automaton(this: *mut u8, first: u32, second: u32) -> u32 = "thiscall" 0x0050_C990; }

layout! {
    /// Controller fields used by its destructor, from the primary interface.
    struct Controller {
        0x00 => primary_table / set_primary_table: u32,
        0x04 => secondary_table / set_secondary_table: u32,
        0x0C => lua / set_lua: *mut u8,
        0x28 => string_table / set_string_table: u32,
        0x2C => string_buffer / set_string_buffer: *mut u8,
        0x4C => retained / set_retained: *mut u8,
    }
}

port! {
    /// Returns the pointer at +0x0C without changing other registers.
    #[port(address = 0x0051_7F70, owner = "shared", name = "GetLuaPointer", naked)]
    pub unsafe extern "C" fn fc4p_00517f70() {
        core::arch::naked_asm!("mov eax, [ecx + 0x0c]", "ret");
    }
}

port! {
    /// Returns the address of the advisor collection at +0x4C.
    #[port(address = 0x0052_0080, owner = "shared", name = "GetAdvisorCollection", naked)]
    pub unsafe extern "C" fn fc4p_00520080() {
        core::arch::naked_asm!("lea eax, [ecx + 0x4c]", "ret");
    }
}

// A test-only target resolver keeps the forwarding entries independent of the target's own port.
#[cfg(test)]
extern "C" fn thunk_target(address: u32) -> usize {
    crate::abi::target(address)
}

macro_rules! adjust_interface {
    ($export:ident, $address:literal, $offset:literal, $target:literal, $declaration:ident, $name:literal) => {
        game_fn! { #[allow(dead_code)] fn $declaration(this: *mut u8) = "thiscall" $target; }
        port! {
            /// Adjusts the interface pointer and forwards the caller's registers and stack arguments.
            #[port(address = $address, owner = "shared", name = $name, naked)]
            pub unsafe extern "C" fn $export() {
                #[cfg(not(test))]
                core::arch::naked_asm!(
                    "sub ecx, {offset}", "jmp dword ptr [2f]", "2: .long {target}",
                    offset = const $offset, target = const $target,
                );
                #[cfg(test)]
                core::arch::naked_asm!(
                    "sub ecx, {offset}", "push eax", "push ecx", "push edx",
                    "push {target}", "call {resolve}", "add esp, 4",
                    "xchg eax, [esp + 8]", "pop edx", "pop ecx", "ret",
                    offset = const $offset, target = const $target, resolve = sym thunk_target,
                );
            }
        }
    };
}

adjust_interface!(
    fc4p_005183b0,
    0x0051_83B0,
    4,
    0x008A_E800,
    retain_primary_interface,
    "AddRefPrimaryFromSecondary"
);
adjust_interface!(
    fc4p_0051b3c0,
    0x0051_B3C0,
    8,
    0x0051_B1C0,
    release_primary_interface,
    "ReleasePrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_005200d0,
    0x0052_00D0,
    4,
    0x0069_5330,
    retain_primary_from_second,
    "AddRefPrimaryFromSecondInterface"
);
adjust_interface!(
    fc4p_005200f0,
    0x0052_00F0,
    8,
    0x0069_5330,
    retain_primary_from_third,
    "AddRefPrimaryFromThirdInterface"
);
adjust_interface!(
    fc4p_00520110,
    0x0052_0110,
    12,
    0x0069_5330,
    retain_primary_from_fourth,
    "AddRefPrimaryFromFourthInterface"
);
adjust_interface!(
    fc4p_005243e0,
    0x0052_43E0,
    12,
    0x0052_4410,
    delete_bus_primary,
    "DeletePrimaryFromFourthInterface"
);

port! {
    /// Destroys a bus, conditionally frees its 0x2B8-byte allocation, and returns the original pointer.
    #[port(address = 0x0052_4410, owner = "shared", name = "DeleteBus")]
    pub unsafe extern "thiscall" fn fc4p_00524410(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game supplies the object and its deleting-destructor flags.
        unsafe {
            destroy_bus(this);
            if flags & 1 != 0 { free_sized_bus(this, 0x2B8); }
        }
        this
    }
}

port! {
    /// Destroys an ambulance and frees it only when the low flag bit is set.
    #[port(address = 0x0053_2270, owner = "shared", name = "DeleteAmbulance")]
    pub unsafe extern "thiscall" fn fc4p_00532270(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game's object and destructor/free pair.
        unsafe {
            destroy_ambulance(this);
            if flags & 1 != 0 { heap_free(this); }
        }
        this
    }
}

port! {
    /// Destroys a mass-transit train and frees it only when the low flag bit is set.
    #[port(address = 0x0053_CCE0, owner = "shared", name = "DeleteMassTransitTrain")]
    pub unsafe extern "thiscall" fn fc4p_0053cce0(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game's object and destructor/free pair.
        unsafe {
            destroy_train(this);
            if flags & 1 != 0 { heap_free(this); }
        }
        this
    }
}

port! {
    /// Writes version 1 before forwarding to the vehicle serializer, regardless of the version result.
    #[port(address = 0x0053_CE00, owner = "shared", name = "WriteVersionedVehicle")]
    pub unsafe extern "thiscall" fn fc4p_0053ce00(this: *mut u8, stream: *mut u8) -> u32 {
        // SAFETY: the original stream and vehicle serializer signatures.
        unsafe {
            vcall!(stream, 0x1C => fn(u32) -> u32, 1);
            write_vehicle(this, stream)
        }
    }
}

port! {
    /// Restores the controller tables, releases the retained object, destroys its list and string,
    /// then releases Lua. Retained pointer fields are left as they were.
    #[port(address = 0x0051_7F80, owner = "shared", name = "DestroyController")]
    pub unsafe extern "fastcall" fn fc4p_00517f80(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original controller and its retained objects.
        unsafe {
            let controller = Controller::from_ptr(this);
            controller.set_primary_table(0x00A9_5D90);
            controller.set_secondary_table(0x00A9_5D74);
            let retained = controller.retained();
            let result = if retained.is_null() {
                registers(0, entry_edx)
            } else {
                let table = word(retained, 0);
                virtual_call(retained, 8, table, entry_edx, &[])
            };
            let mut result = invoke(Conv::Thiscall, crate::abi::target(0x0056_50A0),
                this.add(0x44), result as u32, (result >> 32) as u32, &[]);
            controller.set_string_table(0x00A8_0810);
            let buffer = controller.string_buffer();
            if buffer.is_null() {
                result = registers(0, (result >> 32) as u32);
            } else {
                result = invoke(Conv::Cdecl, crate::abi::target(0x0090_CF63), this.add(0x44),
                    buffer as u32, (result >> 32) as u32, &[buffer as u32]);
            }
            let lua = controller.lua();
            if lua.is_null() { result } else {
                virtual_call(lua, 8, result as u32, word(lua, 0), &[])
            }
        }
    }
}

port! {
    /// Calls virtual shutdown, destroys the controller through its original entry, then frees it.
    #[port(address = 0x0051_9340, owner = "shared", name = "ShutdownAndDeleteController")]
    pub unsafe extern "fastcall" fn fc4p_00519340(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the original primary object and its virtual shutdown slot.
        unsafe {
            let result = virtual_call(this, 0x10, word(this, 0), entry_edx, &[]);
            let result = invoke(Conv::Thiscall, crate::abi::target(0x0051_7F80), this,
                result as u32, (result >> 32) as u32, &[]);
            invoke(Conv::Cdecl, crate::abi::target(0x005E_5620), this,
                result as u32, (result >> 32) as u32, &[this as u32])
        }
    }
}

port! {
    /// Removes an automaton through the base manager, always calls the +0x114 hook, and returns
    /// the removal byte in the hook result's upper EAX bytes, together with the hook's EDX.
    #[port(address = 0x0054_D690, owner = "shared", name = "RemoveAutomatonAndNotify")]
    pub unsafe extern "thiscall" fn fc4p_0054d690(this: *mut u8, first: u32, second: u32) -> u64 {
        // SAFETY: the original manager and its removal and notification signatures.
        unsafe {
            let removed = remove_automaton(this, first, second);
            let result = virtual_call(this, 0x114, removed, word(this, 0), &[first]);
            (result & !0xFF) | u64::from(removed as u8)
        }
    }
}

port! {
    /// Calculates the inner square at max(radius - 1, 0) and the outer square at radius.
    /// Integer coordinates and extents wrap. AL is one; the upper EAX bytes retain the argument pointer.
    #[port(address = 0x0054_EC60, owner = "shared", name = "CalculateSquareBounds")]
    pub unsafe extern "stdcall" fn fc4p_0054ec60(context: *mut u8) -> u32 {
        // SAFETY: the caller supplies the eleven-word square context.
        unsafe {
            let x = word(context, 0);
            let z = word(context, 4);
            let radius = word(context, 8);
            let inner = (radius.wrapping_sub(1) as i32).max(0) as u32;
            store(context, 0x0C, x.wrapping_sub(inner));
            store(context, 0x10, z.wrapping_sub(inner));
            store(context, 0x18, z.wrapping_add(inner));
            store(context, 0x14, x.wrapping_add(inner));
            store(context, 0x1C, x.wrapping_sub(radius));
            store(context, 0x20, z.wrapping_sub(radius));
            store(context, 0x24, x.wrapping_add(radius));
            store(context, 0x28, z.wrapping_add(radius));
            (context as u32 & !0xFF) | 1
        }
    }
}

port! {
    /// Copies the square context word by word, retains its interface, then copies the flag and tail.
    /// Padding at +0x31..+0x33 is not copied; fields after the retain call are read after that call.
    #[port(address = 0x0054_ED10, owner = "shared", name = "CopySquareContext")]
    pub unsafe extern "thiscall" fn fc4p_0054ed10(this: *mut u8, source: *mut u8) -> *mut u8 {
        // SAFETY: both pointers cover the original context layout; overlapping contexts copy forward.
        unsafe {
            for offset in (0..=0x2C).step_by(4) { store(this, offset, word(source, offset)); }
            let retained = word(source, 0x2C) as *mut u8;
            if !retained.is_null() { vcall!(retained, 4 => fn() -> u32); }
            this.add(0x30).write(source.add(0x30).read());
            store(this, 0x34, word(source, 0x34));
        }
        this
    }
}

/// This module's replacements.
pub const PORTS: &[Port] = &[
    fc4p_00517f70::PORT,
    fc4p_00517f80::PORT,
    fc4p_005183b0::PORT,
    fc4p_00519340::PORT,
    fc4p_0051b3c0::PORT,
    fc4p_00520080::PORT,
    fc4p_005200d0::PORT,
    fc4p_005200f0::PORT,
    fc4p_00520110::PORT,
    fc4p_005243e0::PORT,
    fc4p_00524410::PORT,
    fc4p_00532270::PORT,
    fc4p_0053cce0::PORT,
    fc4p_0053ce00::PORT,
    fc4p_0054d690::PORT,
    fc4p_0054ec60::PORT,
    fc4p_0054ed10::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{fake, Object};
    use std::cell::RefCell;

    const RESULT: u64 = 0x7654_3210_ABCD_EF80;

    #[derive(Debug, PartialEq, Eq)]
    struct Call(u32, usize, Vec<u32>);

    type Mutation = (u32, *mut u8, usize, u32);
    thread_local! {
        static CALLS: RefCell<Vec<Call>> = const { RefCell::new(Vec::new()) };
        static MUTATION: RefCell<Option<Mutation>> = const { RefCell::new(None) };
    }

    fn log(id: u32, this: *mut u8, args: &[u32]) {
        CALLS.with(|calls| calls.borrow_mut().push(Call(id, this as usize, args.to_vec())));
        MUTATION.with(|mutation| {
            let mut mutation = mutation.borrow_mut();
            if let Some((trigger, object, offset, value)) = *mutation {
                if id == trigger {
                    // SAFETY: the test installs a writable fixture field.
                    unsafe { store(object, offset, value) };
                    *mutation = None;
                }
            }
        });
    }

    fn calls() -> Vec<Call> {
        CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()))
    }

    fn mutation(trigger: u32, object: *mut u8, offset: usize, value: u32) {
        MUTATION.with(|m| *m.borrow_mut() = Some((trigger, object, offset, value)));
    }

    fn bytes(object: *mut u8, size: usize) -> Vec<u8> {
        // SAFETY: every use names a live fixture and its allocated size.
        unsafe { core::slice::from_raw_parts(object, size).to_vec() }
    }

    fn run(entry: *const (), this: *mut u8, args: &[u32], eax: u32, edx: u32) -> u64 {
        // SAFETY: each test passes the entry's original stack words and a live fixture.
        unsafe { invoke(Conv::Thiscall, entry as usize, this, eax, edx, args) }
    }

    #[test]
    fn getters_return_the_field_or_collection_address_without_writes() {
        let mut object = Object::new(0x60);
        let p = object.ptr();
        unsafe { store(p, 0x0C, 0xF123_4567) };
        let before = bytes(p, 0x60);
        assert_eq!(
            run(fc4p_00517f70 as *const (), p, &[], 7, 0x1234),
            registers(0xF123_4567, 0x1234)
        );
        assert_eq!(
            run(fc4p_00520080 as *const (), p, &[], 9, 0xABCD),
            registers(p as u32 + 0x4C, 0xABCD)
        );
        assert_eq!(bytes(p, 0x60), before);
        assert_eq!(
            run(fc4p_00520080 as *const (), 0xFFFF_FFF0usize as *mut u8, &[], 0, 9),
            registers(0x3C, 9)
        );
    }

    #[unsafe(naked)]
    unsafe extern "C" fn forwarded() {
        core::arch::naked_asm!("xor eax, [esp + 4]", "add eax, [esp + 8]", "xor edx, ecx", "ret 8");
    }

    #[test]
    fn interface_adjustments_forward_every_stack_word_and_return_register() {
        let entries = [
            (fc4p_005183b0 as *const (), 4u32, 0x008A_E800),
            (fc4p_0051b3c0 as *const (), 8, 0x0051_B1C0),
            (fc4p_005200d0 as *const (), 4, 0x0069_5330),
            (fc4p_005200f0 as *const (), 8, 0x0069_5330),
            (fc4p_00520110 as *const (), 12, 0x0069_5330),
            (fc4p_005243e0 as *const (), 12, 0x0052_4410),
        ];
        for (entry, adjustment, target) in entries {
            let _fake = fake(target, forwarded as *const () as usize);
            for this in [0u32, 4, 0x1234_5678, u32::MAX] {
                let result = run(
                    entry,
                    this as *mut u8,
                    &[0xFEDC_BA98, 0x5432_1001],
                    0x1234_5678,
                    0xAABB_CCDD,
                );
                assert_eq!(
                    result,
                    registers(
                        (0x1234_5678u32 ^ 0xFEDC_BA98).wrapping_add(0x5432_1001),
                        0xAABB_CCDD ^ this.wrapping_sub(adjustment)
                    )
                );
            }
        }
    }

    extern "thiscall" fn bus_destroy(this: *mut u8) {
        log(0x0050_4E70, this, &[]);
    }
    extern "thiscall" fn ambulance_destroy(this: *mut u8) {
        log(0x0050_4EF0, this, &[]);
    }
    extern "thiscall" fn train_destroy(this: *mut u8) {
        log(0x0053_CB80, this, &[]);
    }
    extern "C" fn free(this: *mut u8) -> u64 {
        log(0x005E_5620, this, &[]);
        RESULT
    }
    extern "C" fn free_array(this: *mut u8) -> u64 {
        log(0x0090_CF63, this, &[]);
        RESULT
    }
    extern "C" fn sized_free(this: *mut u8, size: u32) {
        log(0x0055_CA60, this, &[size]);
    }

    #[test]
    fn deleting_destructors_use_only_the_low_flag_bit_and_the_original_allocator() {
        let _free = fake(0x005E_5620, free as *const () as usize);
        let _sized = fake(0x0055_CA60, sized_free as *const () as usize);
        let _bus = fake(0x0050_4E70, bus_destroy as *const () as usize);
        let _ambulance = fake(0x0050_4EF0, ambulance_destroy as *const () as usize);
        let _train = fake(0x0053_CB80, train_destroy as *const () as usize);
        let mut object = Object::new(0x20);
        let p = object.ptr();
        let before = bytes(p, 0x20);
        for (entry, destroy, size) in [
            (fc4p_00524410 as *const (), 0x0050_4E70, Some(0x2B8)),
            (fc4p_00532270 as *const (), 0x0050_4EF0, None),
            (fc4p_0053cce0 as *const (), 0x0053_CB80, None),
        ] {
            for flags in [0, 1, 2, 3, 0x100, 0xFFFF_FFFE, u32::MAX] {
                let result = run(entry, p, &[flags], 0, 0);
                assert_eq!(result as u32, p as u32);
                let mut expected = vec![Call(destroy, p as usize, vec![])];
                if flags & 1 != 0 {
                    expected.push(match size {
                        Some(size) => Call(0x0055_CA60, p as usize, vec![size]),
                        None => Call(0x005E_5620, p as usize, vec![]),
                    });
                }
                assert_eq!(calls(), expected);
                assert_eq!(bytes(p, 0x20), before);
            }
        }
    }

    extern "thiscall" fn version(this: *mut u8, version: u32) -> u32 {
        log(0x1C, this, &[version]);
        0
    }

    extern "thiscall" fn vehicle_write(this: *mut u8, stream: *mut u8) -> u32 {
        log(0x0055_F640, this, &[stream as u32]);
        0x1234_AB80
    }

    #[test]
    fn versioned_vehicle_write_forwards_even_when_the_version_writer_returns_zero() {
        let _write = fake(0x0055_F640, vehicle_write as *const () as usize);
        let mut table = [0usize; 8];
        table[7] = version as *const () as usize;
        let mut stream = [table.as_ptr() as usize];
        let stream = stream.as_mut_ptr().cast::<u8>();
        let this = 0x1234usize as *mut u8;
        assert_eq!(unsafe { fc4p_0053ce00(this, stream) }, 0x1234_AB80);
        assert_eq!(
            calls(),
            [
                Call(0x1C, stream as usize, vec![1]),
                Call(0x0055_F640, this as usize, vec![stream as u32])
            ]
        );
    }

    extern "thiscall" fn release(this: *mut u8) -> u64 {
        log(8, this, &[]);
        RESULT
    }
    extern "thiscall" fn retain(this: *mut u8) -> u32 {
        log(4, this, &[]);
        99
    }
    extern "thiscall" fn list_destroy(this: *mut u8) -> u64 {
        log(0x0056_50A0, this, &[]);
        RESULT
    }
    extern "thiscall" fn controller_destroy(this: *mut u8) -> u64 {
        log(0x0051_7F80, this, &[]);
        RESULT
    }
    extern "thiscall" fn shutdown(this: *mut u8) -> u64 {
        log(0x10, this, &[]);
        RESULT
    }

    #[test]
    fn controller_destruction_restores_tables_and_preserves_retained_pointer_fields() {
        let _list = fake(0x0056_50A0, list_destroy as *const () as usize);
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let mut table = [0usize; 3];
        table[2] = release as *const () as usize;
        let mut retained = [table.as_ptr() as usize];
        let retained = retained.as_mut_ptr().cast::<u8>();
        let mut lua = [table.as_ptr() as usize];
        let lua = lua.as_mut_ptr().cast::<u8>();
        let mut object = Object::new(0x60);
        let p = object.ptr();
        for with_retained in [false, true] {
            for with_buffer in [false, true] {
                for with_lua in [false, true] {
                    unsafe {
                        core::ptr::write_bytes(p, 0xA5, 0x60);
                        let c = Controller::from_ptr(p);
                        c.set_retained(if with_retained { retained } else { core::ptr::null_mut() });
                        c.set_lua(if with_lua { lua } else { core::ptr::null_mut() });
                        c.set_string_buffer(if with_buffer {
                            0x1122usize as *mut u8
                        } else {
                            core::ptr::null_mut()
                        });
                    }
                    let mut expected_bytes = bytes(p, 0x60);
                    for (offset, value) in [(0, 0x00A9_5D90u32), (4, 0x00A9_5D74), (0x28, 0x00A8_0810)] {
                        expected_bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
                    }
                    let result = run(fc4p_00517f80 as *const (), p, &[], 0x9988_7766, 0x1234);
                    let mut expected = vec![];
                    if with_retained {
                        expected.push(Call(8, retained as usize, vec![]));
                    }
                    expected.push(Call(0x0056_50A0, p as usize + 0x44, vec![]));
                    if with_buffer {
                        expected.push(Call(0x0090_CF63, 0x1122, vec![]));
                    }
                    if with_lua {
                        expected.push(Call(8, lua as usize, vec![]));
                    }
                    assert_eq!(calls(), expected);
                    assert_eq!(bytes(p, 0x60), expected_bytes);
                    assert_eq!(result as u32, if with_buffer || with_lua { RESULT as u32 } else { 0 });
                }
            }
        }
    }

    #[test]
    fn controller_reloads_buffer_after_list_destruction() {
        let _list = fake(0x0056_50A0, list_destroy as *const () as usize);
        let _free = fake(0x0090_CF63, free_array as *const () as usize);
        let mut object = Object::new(0x60);
        let p = object.ptr();
        mutation(0x0056_50A0, p, 0x2C, 0x4455);
        assert_eq!(run(fc4p_00517f80 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x0056_50A0, p as usize + 0x44, vec![]),
                Call(0x0090_CF63, 0x4455, vec![])
            ]
        );
    }

    #[test]
    fn controller_delete_calls_shutdown_destroy_and_free_in_order() {
        let _destroy = fake(0x0051_7F80, controller_destroy as *const () as usize);
        let _free = fake(0x005E_5620, free as *const () as usize);
        let mut table = [0usize; 5];
        table[4] = shutdown as *const () as usize;
        let mut object = [table.as_ptr() as usize];
        let p = object.as_mut_ptr().cast();
        assert_eq!(run(fc4p_00519340 as *const (), p, &[], 0, 0), RESULT);
        assert_eq!(
            calls(),
            [
                Call(0x10, p as usize, vec![]),
                Call(0x0051_7F80, p as usize, vec![]),
                Call(0x005E_5620, p as usize, vec![])
            ]
        );
    }

    extern "thiscall" fn remove(this: *mut u8, first: u32, second: u32) -> u32 {
        log(0x0050_C990, this, &[first, second]);
        second
    }
    extern "thiscall" fn removed(this: *mut u8, first: u32) -> u64 {
        log(0x114, this, &[first]);
        RESULT
    }

    #[test]
    fn removal_hook_always_runs_and_only_its_low_eax_byte_is_replaced() {
        let _remove = fake(0x0050_C990, remove as *const () as usize);
        let mut table = [0usize; 0x118 / 4];
        table[0x114 / 4] = removed as *const () as usize;
        let mut object = [table.as_ptr() as usize];
        let p = object.as_mut_ptr().cast();
        for second in [0, 1, 0x1234_5600, 0x1234_56FE] {
            assert_eq!(
                run(fc4p_0054d690 as *const (), p, &[0x1122_3344, second], 0, 0),
                (RESULT & !0xFF) | u64::from(second as u8)
            );
            assert_eq!(
                calls(),
                [
                    Call(0x0050_C990, p as usize, vec![0x1122_3344, second]),
                    Call(0x114, p as usize, vec![0x1122_3344])
                ]
            );
        }
    }

    #[test]
    fn square_bounds_cover_zero_negative_and_wrapping_extents_without_extra_writes() {
        let mut object = Object::new(0x40);
        let p = object.ptr();
        for (x, z, radius, inner) in [
            (10u32, 20u32, 3u32, 2u32),
            (0, 0, 0, 0),
            (5, 7, 1, 0),
            (0, u32::MAX, u32::MAX, 0),
            (0x8000_0000, 0x7FFF_FFFF, 0x8000_0000, 0x7FFF_FFFF),
        ] {
            unsafe {
                core::ptr::write_bytes(p, 0xA5, 0x40);
                store(p, 0, x);
                store(p, 4, z);
                store(p, 8, radius);
            }
            let mut expected = bytes(p, 0x40);
            for (index, value) in [
                x.wrapping_sub(inner),
                z.wrapping_sub(inner),
                x.wrapping_add(inner),
                z.wrapping_add(inner),
                x.wrapping_sub(radius),
                z.wrapping_sub(radius),
                x.wrapping_add(radius),
                z.wrapping_add(radius),
            ]
            .into_iter()
            .enumerate()
            {
                expected[12 + index * 4..16 + index * 4].copy_from_slice(&value.to_le_bytes());
            }
            assert_eq!(unsafe { fc4p_0054ec60(p) }, (p as u32 & !0xFF) | 1);
            assert_eq!(bytes(p, 0x40), expected);
        }
    }

    #[test]
    fn context_copy_retains_before_reading_the_flag_and_tail_and_leaves_padding() {
        let mut table = [0usize; 2];
        table[1] = retain as *const () as usize;
        let mut retained = [table.as_ptr() as usize];
        let retained = retained.as_mut_ptr().cast::<u8>();
        let mut source = Object::new(0x40);
        let mut dest = Object::new(0x40);
        let (s, d) = (source.ptr(), dest.ptr());
        for pointer in [0, retained as u32] {
            unsafe {
                for offset in 0..0x40 {
                    s.add(offset).write(offset as u8);
                }
                core::ptr::write_bytes(d, 0xA5, 0x40);
                store(s, 0x2C, pointer);
            }
            if pointer != 0 {
                mutation(4, s, 0x30, 0xDEAD_BE78);
            }
            let result = unsafe { fc4p_0054ed10(d, s) };
            assert_eq!(result, d);
            let mut expected = vec![0xA5; 0x40];
            expected[..0x30].copy_from_slice(&bytes(s, 0x30));
            expected[0x30] = unsafe { s.add(0x30).read() };
            expected[0x34..0x38].copy_from_slice(&bytes(s.wrapping_add(0x34), 4));
            assert_eq!(bytes(d, 0x40), expected);
            assert_eq!(
                calls(),
                if pointer == 0 {
                    vec![]
                } else {
                    vec![Call(4, retained as usize, vec![])]
                }
            );
        }
    }

    #[test]
    fn context_copy_supports_self_assignment_and_forward_overlap() {
        let mut storage = Object::new(0x80);
        let p = storage.ptr();
        for delta in [0usize, 4] {
            unsafe {
                core::ptr::write_bytes(p, 0, 0x80);
                for offset in (4..0x2C).step_by(4) {
                    store(p, offset, offset as u32 + 1);
                }
            }
            let mut expected = bytes(p, 0x80);
            for offset in (0..0x30).step_by(4) {
                let value: [u8; 4] = expected[offset..offset + 4].try_into().unwrap();
                expected[delta + offset..delta + offset + 4].copy_from_slice(&value);
            }
            expected[delta + 0x30] = expected[0x30];
            let tail: [u8; 4] = expected[0x34..0x38].try_into().unwrap();
            expected[delta + 0x34..delta + 0x38].copy_from_slice(&tail);
            unsafe { fc4p_0054ed10(p.add(delta), p) };
            assert_eq!(bytes(p, 0x80), expected);
        }
        assert!(calls().is_empty());
    }
}
