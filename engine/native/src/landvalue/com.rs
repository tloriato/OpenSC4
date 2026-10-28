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

//! The land value simulator's interfaces, lifetime, queries and cycle notification.

use super::*;

/// The cell getter also leaves caller-visible ECX and EDX values. Capture them at the virtual call itself;
/// its hidden EDX input is x, in addition to the two ordinary stack arguments.
unsafe fn cell_registers(grid: *mut u8, x: i32, z: i32) -> (u32, u32, u32) {
    // SAFETY: the caller supplies the total grid with its cell getter at +0x1C.
    let target = unsafe { crate::abi::vslot(grid, 0x1C) };
    let eax: u32;
    let mut ecx = grid as u32;
    let mut edx = x as u32;
    // SAFETY: thiscall, two stack words removed by the callee, plus the original's EDX input. Every
    // caller-saved register is declared, and the three integer results are captured before Rust resumes.
    unsafe {
        core::arch::asm!("push {z}", "push edx", "call {target}",
            z = in(reg) z, target = in(reg) target,
            lateout("eax") eax, inout("ecx") ecx, inout("edx") edx, clobber_abi("C"));
    }
    (eax, ecx, edx)
}

port! {
    /// The simulator's class identifier, independent of the interface pointer.
    #[port(address = 0x006B_D4B0, owner = "landvalue", name = "cSC4LandValueSimulator::GetGZCLSID")]
    pub unsafe extern "thiscall" fn fc4p_006bd4b0(_this: *mut u8) -> u32 {
        0xE990_C18E
    }
}

port! {
    /// Answers the primary, unknown, message-target and serializable IIDs. Writes the interface before
    /// calling the primary interface's AddRef. Only AL is replaced in that call's EAX result.
    /// Unsupported IIDs leave the output untouched and retain the comparison arithmetic's upper EAX bytes.
    /// The hidden EDX input and EDX:EAX result preserve EDX on rejection and the AddRef's EDX on success;
    /// the stack arguments and eight-byte callee pop are those of the original thiscall entry.
    #[port(address = 0x006B_D4C0, owner = "landvalue", name = "cSC4LandValueSimulator::QueryInterface")]
    pub unsafe extern "fastcall" fn fc4p_006bd4c0(this: *mut u8, entry_edx: u32, iid: u32, out: *mut *mut u8) -> u64 {
        let offset = match iid {
            1 | 0xE778_1D8C => 0,
            0x4522_94AA => 4,
            0xE4FD_A3D4 => 8,
            _ => {
                let eax = if iid > 0xE4FD_A3D4 { iid } else { iid.wrapping_sub(0x4522_94AA) };
                return (u64::from(entry_edx) << 32) | u64::from(eax & !0xFF);
            }
        };
        let interface = if this.is_null() { this } else { this.wrapping_add(offset) };
        // SAFETY: the caller supplies the output word and a live primary interface on success.
        // A null primary pointer still writes null before attempting the same virtual call.
        unsafe {
            out.write_unaligned(interface);
            let vtable = this.cast::<u32>().read_unaligned();
            let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                crate::abi::vslot(this, 0x04), this as u32, &[]);
            call.eax = vtable;
            call.edx = out as u32;
            crate::abi::call_raw(&mut call);
            (u64::from(call.edx) << 32) | u64::from((call.eax & !0xFF) | 1)
        }
    }
}

port! {
    /// Allocates and constructs a standard message, retains it, sets type 0x87CC75B3, posts it with a null
    /// target, then releases it. The final Release's EAX is left for the caller.
    #[port(address = 0x006B_D6B0, owner = "landvalue", name = "cSC4LandValueSimulator::DoEndOfSimulationCycle")]
    pub unsafe extern "thiscall" fn fc4p_006bd6b0(this: *mut u8) -> u32 {
        // SAFETY: the game supplies the simulator and its message server; allocation and construction use
        // the game's own functions, including the allocation-failure path's null virtual call.
        unsafe {
            let lv = LandValueSim::from_ptr(this);
            let storage = game::message_allocate(0x2C);
            let message = if storage.is_null() { storage } else { game::message_construct(storage) };
            vcall!(message, 0x04 => fn() -> u32);
            vcall!(message, 0x14 => fn(u32) -> u32, 0x87CC_75B3);
            vcall!(lv.message_server(), 0x10 => fn(*mut u8, u32) -> u32, message, 0);
            vcall!(message, 0x08 => fn() -> u32)
        }
    }
}

port! {
    /// Reads total land value in cell coordinates and compares its low byte with the two unsigned wealth
    /// thresholds. The thresholds are read after the grid call, and the low threshold takes precedence.
    #[port(address = 0x006B_D700, owner = "landvalue", name = "cSC4LandValueSimulator::GetLandValueType", naked)]
    pub unsafe extern "C" fn fc4p_006bd700() {
        // The Rust body supplies EAX, ECX and EDX in a private stack buffer. The entry only marshals
        // arguments and result registers, since the ordinary Rust ABI has no ECX return channel.
        core::arch::naked_asm!("sub esp, 12", "mov edx, esp",
            "push [esp + 20]", "push [esp + 20]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]",
            "add esp, 12", "ret 8", sym land_value_type);
    }
}

unsafe extern "fastcall" fn land_value_type(this: *mut u8, registers: *mut [u32; 3], x: i32, z: i32) {
    // SAFETY: the game's simulator and its total grid, with the entry's private result buffer.
    unsafe {
        let lv = LandValueSim::from_ptr(this);
        let (eax, ecx, edx) = cell_registers(lv.total_grid(), x, z);
        let value = eax as u8;
        let result = if value < lv.low_wealth_boundary() {
            1
        } else if value < lv.high_wealth_boundary() {
            2
        } else {
            3
        };
        registers.write([result, ecx, edx]);
    }
}

port! {
    /// Forwards the original stack arguments to the total grid's cell getter, preserving its return registers.
    #[port(address = 0x006B_D740, owner = "landvalue", name = "cSC4LandValueSimulator::GetLandValue", naked)]
    pub unsafe extern "C" fn fc4p_006bd740() {
        core::arch::naked_asm!("mov ecx, [ecx + 0x28]", "mov eax, [ecx]", "jmp dword ptr [eax + 0x1c]");
    }
}

port! {
    /// Decrements counts greater than one using a signed comparison. Otherwise calls Shutdown, then the
    /// deleting destructor with flag 1, rereading the vtable between calls, and returns zero.
    /// The hidden EDX argument preserves its entry value when no call is made; otherwise the destructor's
    /// EDX is returned alongside EAX. No stack arguments are added to the original thiscall entry.
    #[port(address = 0x006B_D8A0, owner = "landvalue", name = "cSC4LandValueSimulator::Release")]
    pub unsafe extern "fastcall" fn fc4p_006bd8a0(this: *mut u8, entry_edx: u32) -> u64 {
        // SAFETY: the game calls this on a live simulator.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        let count = lv.reference_count();
        if count > 1 {
            let next = count.wrapping_sub(1);
            lv.set_reference_count(next);
            (u64::from(entry_edx) << 32) | u64::from(next as u32)
        } else {
            // SAFETY: the simulator's virtual Shutdown and deleting destructor.
            unsafe {
                let shutdown_eax = vcall!(this, 0x10 => fn() -> u32);
                let vtable = this.cast::<u32>().read_unaligned();
                let mut call = crate::abi::RawCall::new(crate::abi::Conv::Thiscall,
                    crate::abi::vslot(this, 0x30), this as u32, &[1]);
                call.eax = shutdown_eax;
                call.edx = vtable;
                crate::abi::call_raw(&mut call);
                u64::from(call.edx) << 32
            }
        }
    }
}

port! {
    /// Loads only AL; every other return-register bit retains its entry value.
    #[port(address = 0x006B_DD90, owner = "landvalue", name = "cSC4LandValueSimulator::GetAverageLandValue", naked)]
    pub unsafe extern "C" fn fc4p_006bdd90() {
        core::arch::naked_asm!("mov al, [ecx + 0x74]", "ret");
    }
}

port! {
    /// Loads the stored mode altitude directly onto the caller's x87 stack.
    #[port(address = 0x006B_DDA0, owner = "landvalue", name = "cSC4LandValueSimulator::GetModeAltitude", naked)]
    pub unsafe extern "C" fn fc4p_006bdda0() {
        core::arch::naked_asm!("fld dword ptr [ecx + 0xa0]", "ret");
    }
}

port! {
    /// Adjusts the message-target interface to the primary interface and forwards QueryInterface.
    #[port(address = 0x006B_DDB0, owner = "landvalue", name = "cSC4LandValueSimulator::MessageTargetQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006bddb0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x006bd4c0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym fc4p_006bd4c0);
    }
}

port! {
    /// Adjusts the serializable interface to the primary interface and forwards QueryInterface.
    #[port(address = 0x006B_DDC0, owner = "landvalue", name = "cSC4LandValueSimulator::SerializableQueryInterface", naked)]
    pub unsafe extern "C" fn fc4p_006bddc0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 8", "jmp dword ptr [2f]", "2: .long 0x006bd4c0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 8", "jmp {}", sym fc4p_006bd4c0);
    }
}

port! {
    /// Adjusts the message-target interface to the primary interface and forwards Release.
    #[port(address = 0x006B_DDD0, owner = "landvalue", name = "cSC4LandValueSimulator::MessageTargetRelease", naked)]
    pub unsafe extern "C" fn fc4p_006bddd0() {
        #[cfg(not(test))]
        core::arch::naked_asm!("sub ecx, 4", "jmp dword ptr [2f]", "2: .long 0x006bd8a0");
        #[cfg(test)]
        core::arch::naked_asm!("sub ecx, 4", "jmp {}", sym fc4p_006bd8a0);
    }
}

port! {
    /// Destroys the simulator's containers, frees the object if the low flag bit is set, and returns the
    /// original object pointer. Other flag bits do not affect freeing.
    #[port(address = 0x006B_DED0, owner = "landvalue", name = "cSC4LandValueSimulator::DeletingDestructor")]
    pub unsafe extern "thiscall" fn fc4p_006bded0(this: *mut u8, flags: u32) -> *mut u8 {
        // SAFETY: the game's own destruction and heap functions operate on the supplied simulator.
        unsafe {
            game::land_value_destroy(this);
            if flags & 1 != 0 {
                game::heap_free(this);
            }
        }
        this
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[
    fc4p_006bd4b0::PORT,
    fc4p_006bd4c0::PORT,
    fc4p_006bd6b0::PORT,
    fc4p_006bd700::PORT,
    fc4p_006bd740::PORT,
    fc4p_006bd8a0::PORT,
    fc4p_006bdd90::PORT,
    fc4p_006bdda0::PORT,
    fc4p_006bddb0::PORT,
    fc4p_006bddc0::PORT,
    fc4p_006bddd0::PORT,
    fc4p_006bded0::PORT,
];

#[cfg(test)]
mod tests {
    use super::*;
    use crate::abi::{call_raw, Conv, RawCall};
    use crate::testing::{call_thiscall_with_eax, fake, GameFpu, Object};
    use std::cell::RefCell;

    #[derive(Debug, PartialEq)]
    enum Call {
        AddRef(usize),
        Shutdown(usize),
        Delete(usize, u32),
        Destroy(usize),
        Free(usize),
        Cell(usize, i32, i32),
        Allocate(u32),
        Construct(usize),
        RetainMessage(usize),
        SetType(usize, u32),
        Post(usize, usize, u32),
        ReleaseMessage(usize),
    }

    #[derive(Default)]
    struct State {
        calls: Vec<Call>,
        output: usize,
        interface: usize,
        next_vtable: usize,
        storage: usize,
        message: usize,
        simulator: usize,
        server: usize,
    }

    thread_local! {
        static STATE: RefCell<State> = RefCell::new(State::default());
    }

    fn log(call: Call) {
        STATE.with(|s| s.borrow_mut().calls.push(call));
    }

    fn reset() {
        STATE.with(|s| *s.borrow_mut() = State::default());
    }

    fn calls() -> Vec<Call> {
        STATE.with(|s| std::mem::take(&mut s.borrow_mut().calls))
    }

    fn table(slots: &[(usize, usize)]) -> Box<[usize; 16]> {
        let mut table = Box::new([0; 16]);
        for &(offset, function) in slots {
            table[offset / 4] = function;
        }
        table
    }

    fn simulator(object: &mut Object) -> LandValueSim {
        // SAFETY: test objects reserve the full simulator layout.
        unsafe { LandValueSim::from_ptr(object.ptr()) }
    }

    unsafe fn type_value(this: *mut u8, x: i32, z: i32) -> u32 {
        // SAFETY: the caller provides a simulator with a live total grid.
        unsafe { call_thiscall_with_eax(fc4p_006bd700 as *const () as usize, this, &[x as u32, z as u32], 0) }.0
    }

    fn bytes(object: &mut Object) -> Vec<u8> {
        // SAFETY: test simulator objects have 0x100 bytes.
        unsafe { std::slice::from_raw_parts(object.ptr(), 0x100) }.to_vec()
    }

    extern "fastcall" fn add_ref(this: *mut u8, entry_edx: usize) -> u64 {
        log(Call::AddRef(this as usize));
        STATE.with(|s| {
            let s = s.borrow();
            assert_eq!(entry_edx, s.output);
            // SAFETY: the test's output word remains live across the virtual call.
            assert_eq!(unsafe { *(s.output as *const usize) }, s.interface);
        });
        // SAFETY: the test's primary simulator.
        let lv = unsafe { LandValueSim::from_ptr(this) };
        let count = lv.reference_count().wrapping_add(1);
        lv.set_reference_count(count);
        0x7654_3210_0000_0000 | u64::from(count as u32)
    }

    extern "thiscall" fn shutdown(this: *mut u8) -> u32 {
        log(Call::Shutdown(this as usize));
        let next = STATE.with(|s| s.borrow().next_vtable);
        if next != 0 {
            // SAFETY: the test's primary simulator and replacement vtable.
            unsafe { this.cast::<usize>().write(next) };
        }
        0xABCD_EF01
    }

    extern "fastcall" fn delete(this: *mut u8, entry_edx: usize, flags: u32) -> u64 {
        log(Call::Delete(this as usize, flags));
        // SAFETY: the test's primary interface vtable word.
        assert_eq!(entry_edx, unsafe { this.cast::<usize>().read() });
        0x7654_3210_0000_0000 | this as u64
    }

    #[test]
    fn class_id_does_not_read_the_interface() {
        // SAFETY: this function does not access its interface pointer.
        assert_eq!(unsafe { fc4p_006bd4b0(core::ptr::null_mut()) }, 0xE990_C18E);
    }

    #[test]
    fn query_writes_each_interface_before_adding_a_shared_reference() {
        reset();
        let vt = table(&[(4, add_ref as *const () as usize)]);
        let mut object = Object::new(0x100);
        let lv = simulator(&mut object);
        // SAFETY: the object's first word is its vtable.
        unsafe { object.ptr().cast::<usize>().write(vt.as_ptr() as usize) };
        for (iid, offset) in [(1, 0), (0xE778_1D8C, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
            let mut out = core::ptr::null_mut();
            lv.set_reference_count(0x0012_34FE);
            STATE.with(|s| {
                s.borrow_mut().output = &mut out as *mut *mut u8 as usize;
                s.borrow_mut().interface = object.ptr() as usize + offset;
            });
            let mut expected = bytes(&mut object);
            expected[0x10..0x14].copy_from_slice(&0x0012_34FFu32.to_le_bytes());
            // SAFETY: live primary object, output and AddRef vtable slot.
            assert_eq!(
                unsafe { fc4p_006bd4c0(object.ptr(), 0, iid, &mut out) },
                0x7654_3210_0012_3401
            );
            assert_eq!(out as usize, object.ptr() as usize + offset);
            assert_eq!(calls(), [Call::AddRef(object.ptr() as usize)]);
            assert_eq!(bytes(&mut object), expected);
        }
    }

    #[test]
    fn rejected_iids_keep_output_and_comparison_upper_bytes() {
        reset();
        let mut object = Object::new(0x100);
        let before = bytes(&mut object);
        for iid in [
            0u32,
            2,
            0x4522_94A9,
            0x4522_94AB,
            0xE4FD_A3D3,
            0xE4FD_A3D5,
            0xE778_1D8B,
            u32::MAX,
        ] {
            let mut out = 0x1234usize as *mut u8;
            let upper = if iid > 0xE4FD_A3D4 {
                iid
            } else {
                iid.wrapping_sub(0x4522_94AA)
            } & !0xFF;
            // SAFETY: rejected IIDs access neither interface nor output.
            assert_eq!(
                unsafe { fc4p_006bd4c0(object.ptr(), 0x7654_3210, iid, &mut out) },
                0x7654_3210_0000_0000 | u64::from(upper)
            );
            assert_eq!(out as usize, 0x1234);
            assert_eq!(
                unsafe { fc4p_006bd4c0(core::ptr::null_mut(), 0x7654_3210, iid, core::ptr::null_mut()) },
                0x7654_3210_0000_0000 | u64::from(upper)
            );
        }
        assert!(calls().is_empty());
        assert_eq!(bytes(&mut object), before);
    }

    #[test]
    fn interface_query_thunks_adjust_this_and_pop_two_words() {
        reset();
        let vt = table(&[(4, add_ref as *const () as usize)]);
        let mut object = Object::new(0x100);
        // SAFETY: test object and its vtable word.
        unsafe { object.ptr().cast::<usize>().write(vt.as_ptr() as usize) };
        for (function, offset) in [
            (fc4p_006bddb0 as *const () as usize, 4),
            (fc4p_006bddc0 as *const () as usize, 8),
        ] {
            for (iid, result_offset) in [(1, 0), (0x4522_94AA, 4), (0xE4FD_A3D4, 8)] {
                let mut out = core::ptr::null_mut();
                STATE.with(|s| {
                    s.borrow_mut().output = &mut out as *mut *mut u8 as usize;
                    s.borrow_mut().interface = object.ptr() as usize + result_offset;
                });
                // SAFETY: the appropriate secondary interface and the original two stack words.
                let result = unsafe {
                    call_thiscall_with_eax(
                        function,
                        object.ptr().add(offset),
                        &[iid, &mut out as *mut *mut u8 as u32],
                        0xFFFF_FFFF,
                    )
                };
                assert_eq!(result.0, 1);
                assert_eq!(out as usize, object.ptr() as usize + result_offset);
                assert_eq!(calls(), [Call::AddRef(object.ptr() as usize)]);
            }
        }
        assert_eq!(simulator(&mut object).reference_count(), 6);
    }

    #[test]
    fn release_only_writes_the_count_above_one() {
        reset();
        let mut object = Object::new(0x100);
        for count in [2i32, 3, 256, i32::MAX] {
            simulator(&mut object).set_reference_count(count);
            let mut expected = bytes(&mut object);
            expected[0x10..0x14].copy_from_slice(&(count - 1).to_le_bytes());
            // SAFETY: these counts do not access a virtual slot.
            assert_eq!(
                unsafe { fc4p_006bd8a0(object.ptr(), 0x7654_3210) },
                0x7654_3210_0000_0000 | u64::from((count - 1) as u32)
            );
            assert_eq!(bytes(&mut object), expected);
        }
        assert!(calls().is_empty());
    }

    #[test]
    fn final_release_uses_signed_count_and_rereads_the_vtable() {
        reset();
        let first = table(&[(0x10, shutdown as *const () as usize)]);
        let second = table(&[(0x30, delete as *const () as usize)]);
        let mut object = Object::new(0x100);
        STATE.with(|s| s.borrow_mut().next_vtable = second.as_ptr() as usize);
        for count in [1i32, 0, -1, i32::MIN] {
            // SAFETY: both vtables live across the call.
            unsafe { object.ptr().cast::<usize>().write(first.as_ptr() as usize) };
            simulator(&mut object).set_reference_count(count);
            let mut expected = bytes(&mut object);
            expected[..4].copy_from_slice(&(second.as_ptr() as u32).to_le_bytes());
            assert_eq!(
                unsafe { fc4p_006bd8a0(object.ptr(), 0xABCD_EF12) },
                0x7654_3210_0000_0000
            );
            let p = object.ptr() as usize;
            assert_eq!(calls(), [Call::Shutdown(p), Call::Delete(p, 1)]);
            assert_eq!(bytes(&mut object), expected);
        }
    }

    #[test]
    fn message_target_release_thunk_uses_primary_reference_count() {
        reset();
        let vt = table(&[
            (0x10, shutdown as *const () as usize),
            (0x30, delete as *const () as usize),
        ]);
        let mut object = Object::new(0x100);
        unsafe { object.ptr().cast::<usize>().write(vt.as_ptr() as usize) };
        simulator(&mut object).set_reference_count(2);
        for result in [1, 0] {
            // SAFETY: message-target this is primary +4, with no stack arguments.
            assert_eq!(
                unsafe { call_thiscall_with_eax(fc4p_006bddd0 as *const () as usize, object.ptr().add(4), &[], 0) }.0,
                result
            );
        }
        let p = object.ptr() as usize;
        assert_eq!(calls(), [Call::Shutdown(p), Call::Delete(p, 1)]);
    }

    extern "thiscall" fn destroy(this: *mut u8) {
        log(Call::Destroy(this as usize));
        // SAFETY: the test simulator, demonstrating that destruction happens before freeing.
        unsafe { LandValueSim::from_ptr(this) }.set_reference_count(-123);
    }

    extern "C" fn free(this: *mut u8) {
        assert_eq!(unsafe { LandValueSim::from_ptr(this) }.reference_count(), -123);
        log(Call::Free(this as usize));
    }

    #[test]
    fn deleting_destructor_tests_only_the_low_flag_bit_and_returns_this() {
        reset();
        let _destroy = fake(0x006B_DD10, destroy as *const () as usize);
        let _free = fake(0x005E_5620, free as *const () as usize);
        let mut object = Object::new(0x100);
        for flags in [0u32, 1, 2, 3, 0x100, 0xFFFF_FFFE, u32::MAX] {
            simulator(&mut object).set_reference_count(42);
            let mut expected = bytes(&mut object);
            expected[0x10..0x14].copy_from_slice(&(-123i32).to_le_bytes());
            // SAFETY: the game's callees resolve to the two test functions above.
            assert_eq!(unsafe { fc4p_006bded0(object.ptr(), flags) }, object.ptr());
            let p = object.ptr() as usize;
            let want = if flags & 1 == 0 {
                vec![Call::Destroy(p)]
            } else {
                vec![Call::Destroy(p), Call::Free(p)]
            };
            assert_eq!(calls(), want);
            assert_eq!(bytes(&mut object), expected);
        }
    }

    #[repr(C)]
    struct Grid {
        vtable: *const usize,
        values: [u8; 6],
        constant: Option<u32>,
        change: Option<(*mut u8, u8, u8)>,
    }

    extern "thiscall" fn cell(this: *mut Grid, x: i32, z: i32) -> u64 {
        log(Call::Cell(this as usize, x, z));
        // SAFETY: the test grid.
        let grid = unsafe { &mut *this };
        if let Some((lv, low, high)) = grid.change.take() {
            let lv = unsafe { LandValueSim::from_ptr(lv) };
            lv.set_low_wealth_boundary(low);
            lv.set_high_wealth_boundary(high);
        }
        let value = grid.constant.unwrap_or_else(|| {
            assert!((0..12).contains(&x) && (0..2).contains(&z));
            0xABCD_EF00 | u32::from(grid.values[(x >> 1) as usize])
        });
        0x7654_3210_0000_0000 | u64::from(value)
    }

    #[test]
    fn shipped_wealth_boundaries_and_cell_coordinate_queries() {
        // The core's lv16_01 vectors: boundaries 70/120, total tracts 0/69/70/119/120/255.
        reset();
        let vt = table(&[(0x1C, cell as *const () as usize)]);
        let mut grid = Grid {
            vtable: vt.as_ptr(),
            values: [0, 69, 70, 119, 120, 255],
            constant: None,
            change: None,
        };
        let mut object = Object::new(0x100);
        let lv = simulator(&mut object);
        lv.set_total_grid((&mut grid as *mut Grid).cast());
        lv.set_low_wealth_boundary(70);
        lv.set_high_wealth_boundary(120);
        let before = bytes(&mut object);
        for (x, z, want) in [(0, 0, 1), (3, 1, 1), (4, 0, 2), (7, 1, 2), (8, 0, 3), (11, 1, 3)] {
            // SAFETY: simulator's total grid has the expected slot.
            assert_eq!(unsafe { type_value(object.ptr(), x, z) }, want);
            assert_eq!(calls(), [Call::Cell(&grid as *const Grid as usize, x, z)]);
        }
        for (x, z) in [(6, 0), (7, 0), (6, 1), (7, 1)] {
            // SAFETY: getter forwards the two original cell coordinates and preserves EDX:EAX.
            let result = unsafe {
                call_thiscall_with_eax(
                    fc4p_006bd740 as *const () as usize,
                    object.ptr(),
                    &[x as u32, z as u32],
                    0,
                )
            };
            assert_eq!(result, (0xABCD_EF77, 0x7654_3210));
            assert_eq!(calls(), [Call::Cell(&grid as *const Grid as usize, x, z)]);
        }
        assert_eq!(bytes(&mut object), before);
    }

    #[test]
    fn wealth_comparisons_are_unsigned_and_observe_grid_side_effects() {
        reset();
        let vt = table(&[(0x1C, cell as *const () as usize)]);
        let mut grid = Grid {
            vtable: vt.as_ptr(),
            values: [0; 6],
            constant: Some(0),
            change: None,
        };
        let mut object = Object::new(0x100);
        let lv = simulator(&mut object);
        lv.set_total_grid((&mut grid as *mut Grid).cast());
        for (low, high) in [(0, 0), (255, 255), (200, 128), (128, 200)] {
            for value in [0u8, 127, 128, 199, 200, 254, 255] {
                lv.set_low_wealth_boundary(1);
                lv.set_high_wealth_boundary(2);
                grid.constant = Some(0xFFFF_FF00 | u32::from(value));
                grid.change = Some((object.ptr(), low, high));
                let want = if value < low {
                    1
                } else if value < high {
                    2
                } else {
                    3
                };
                // SAFETY: constant grid accepts every signed coordinate unchanged.
                assert_eq!(unsafe { type_value(object.ptr(), i32::MIN, i32::MAX) }, want);
                assert_eq!(calls(), [Call::Cell(&grid as *const Grid as usize, i32::MIN, i32::MAX)]);
            }
        }
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn register_cell(_this: *mut u8, _x: i32, _z: i32) -> u64 {
        core::arch::naked_asm!("mov eax, edx", "mov ecx, 0x10203040", "mov edx, 0x76543210", "ret 8");
    }

    #[unsafe(naked)]
    unsafe extern "thiscall" fn type_with_ecx(_this: *mut u8, _x: i32, _z: i32) -> u64 {
        core::arch::naked_asm!("push [esp + 8]", "push [esp + 8]", "call {}",
            "mov edx, ecx", "ret 8", sym fc4p_006bd700);
    }

    #[test]
    fn type_keeps_the_grid_calls_ecx_and_edx() {
        let vt = table(&[(0x1C, register_cell as *const () as usize)]);
        let mut grid = [vt.as_ptr() as usize];
        let mut object = Object::new(0x100);
        let lv = simulator(&mut object);
        lv.set_total_grid(grid.as_mut_ptr().cast());
        for (low, high, result) in [(120, 200, 1), (70, 120, 2), (70, 119, 3)] {
            lv.set_low_wealth_boundary(low);
            lv.set_high_wealth_boundary(high);
            // SAFETY: the test grid returns the incoming EDX in EAX and fixed ECX/EDX values.
            let (_, edx) =
                unsafe { call_thiscall_with_eax(fc4p_006bd700 as *const () as usize, object.ptr(), &[119, 456], 0) };
            assert_eq!(edx, 0x7654_3210);
            let pair = unsafe { type_with_ecx(object.ptr(), 119, 456) };
            assert_eq!(pair, 0x1020_3040_0000_0000 | result);
        }
    }

    #[test]
    fn average_changes_only_al() {
        let mut object = Object::new(0x100);
        for value in [0, 1, 127, 128, 255] {
            simulator(&mut object).set_average_land_value(value);
            let before = bytes(&mut object);
            let mut c = RawCall::new(
                Conv::Thiscall,
                fc4p_006bdd90 as *const () as usize,
                object.ptr() as u32,
                &[],
            );
            c.eax = 0xABCD_EF12;
            c.edx = 0x7654_3210;
            // SAFETY: this getter takes no stack arguments.
            unsafe { call_raw(&mut c) };
            assert_eq!((c.eax, c.edx), (0xABCD_EF00 | u32::from(value), 0x7654_3210));
            assert_eq!(bytes(&mut object), before);
        }
    }

    #[test]
    fn mode_returns_the_stored_float_on_x87_and_preserves_integer_registers() {
        let _fpu = GameFpu::new();
        let mut object = Object::new(0x100);
        for bits in [
            0u32,
            0x8000_0000,
            272.0f32.to_bits(),
            1,
            f32::INFINITY.to_bits(),
            0x7FC1_2345,
            0x7F81_2345,
        ] {
            simulator(&mut object).set_mode_altitude(f32::from_bits(bits));
            let before = bytes(&mut object);
            let expected = F80::from_f32(f32::from_bits(bits));
            let mut c = RawCall::new(
                Conv::Thiscall,
                fc4p_006bdda0 as *const () as usize,
                object.ptr() as u32,
                &[],
            );
            c.eax = 0xABCD_EF12;
            c.edx = 0x7654_3210;
            c.take_st0 = 1;
            // SAFETY: this getter takes no arguments and leaves one x87 return value.
            unsafe { call_raw(&mut c) };
            assert_eq!((c.eax, c.edx), (0xABCD_EF12, 0x7654_3210));
            assert_eq!(c.st0, expected);
            assert_eq!(bytes(&mut object), before);
        }
    }

    extern "C" fn allocate(size: u32) -> *mut u8 {
        log(Call::Allocate(size));
        STATE.with(|s| s.borrow().storage as *mut u8)
    }

    extern "thiscall" fn construct(this: *mut u8) -> *mut u8 {
        log(Call::Construct(this as usize));
        STATE.with(|s| s.borrow().message as *mut u8)
    }

    extern "thiscall" fn retain_message(this: *mut u8) -> u32 {
        log(Call::RetainMessage(this as usize));
        0xAAAA_BBBB
    }

    extern "thiscall" fn set_type(this: *mut u8, kind: u32) -> u32 {
        log(Call::SetType(this as usize, kind));
        STATE.with(|s| {
            let s = s.borrow();
            // SAFETY: test simulator and replacement server remain live across the call.
            unsafe { LandValueSim::from_ptr(s.simulator as *mut u8) }.set_message_server(s.server as *mut u8);
        });
        0xCCCC_DDDD
    }

    extern "thiscall" fn post(this: *mut u8, message: *mut u8, target: u32) -> u32 {
        log(Call::Post(this as usize, message as usize, target));
        let next = STATE.with(|s| s.borrow().next_vtable);
        // SAFETY: the message's vtable is writable test storage.
        unsafe { message.cast::<usize>().write(next) };
        0xEEEE_FFFF
    }

    extern "thiscall" fn release_message(this: *mut u8) -> u32 {
        log(Call::ReleaseMessage(this as usize));
        0x1234_5678
    }

    #[test]
    fn cycle_posts_a_constructed_message_in_order_and_rereads_dependencies() {
        reset();
        let _allocate = fake(0x0091_33DA, allocate as *const () as usize);
        let _construct = fake(0x0091_34D6, construct as *const () as usize);
        let initial_vt = table(&[
            (4, retain_message as *const () as usize),
            (0x14, set_type as *const () as usize),
        ]);
        let final_vt = table(&[(8, release_message as *const () as usize)]);
        let server_vt = table(&[(0x10, post as *const () as usize)]);
        let mut message = [initial_vt.as_ptr() as usize; 11];
        let mut server = [server_vt.as_ptr() as usize];
        let mut storage = [0u8; 0x2C];
        let mut object = Object::new(0x100);
        let (m, s, p) = (
            message.as_mut_ptr() as usize,
            server.as_mut_ptr() as usize,
            object.ptr() as usize,
        );
        STATE.with(|state| {
            let mut state = state.borrow_mut();
            state.storage = storage.as_mut_ptr() as usize;
            state.message = m;
            state.simulator = p;
            state.server = s;
            state.next_vtable = final_vt.as_ptr() as usize;
        });
        let mut expected = bytes(&mut object);
        expected[0x14..0x18].copy_from_slice(&(s as u32).to_le_bytes());
        // SAFETY: allocator/constructor fakes return live storage and interface; type setting installs the
        // server, and posting installs the vtable used by the final Release.
        assert_eq!(unsafe { fc4p_006bd6b0(object.ptr()) }, 0x1234_5678);
        assert_eq!(
            calls(),
            [
                Call::Allocate(0x2C),
                Call::Construct(storage.as_mut_ptr() as usize),
                Call::RetainMessage(m),
                Call::SetType(m, 0x87CC_75B3),
                Call::Post(s, m, 0),
                Call::ReleaseMessage(m)
            ]
        );
        assert_eq!(bytes(&mut object), expected);
        assert_eq!(message[0], final_vt.as_ptr() as usize);
        assert_eq!(storage, [0; 0x2C]);
    }
}
