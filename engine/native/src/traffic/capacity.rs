// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! The twelve-element trip-capacity property and its sixteen-to-thirty-two-bit promotion.

use super::*;

const CAPACITY: u32 = 0x8A0B_4100;
game_fn! { fn allocate(size: u32) -> *mut u8 = "C" 0x005F_D840; }
game_fn! { fn construct_property(this: *mut u8, zero: u32) -> *mut u8 = "thiscall" 0x005F_DE50; }
game_fn! { #[allow(dead_code)] fn set_capacity(index: u32) -> u64 = "C" 0x0070_DE70; }

port! {
    /// EDI is the property holder, EBX the value, and the caller pops the single stack index.
    #[port(address = 0x0070_DE70, owner = "traffic", name = "SetTripCapacity", naked)]
    pub unsafe extern "C" fn fc4p_0070de70() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp", "push ebx", "push [esp + 20]", "mov ecx, edi", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret", sym set_trip_capacity);
    }
}
unsafe extern "fastcall" fn set_trip_capacity(holder: *mut u8, out: *mut Registers, index: u32, value: u32) {
    unsafe {
        let mut property = vcall!(holder, 0x24 => fn(u32) -> *mut u8, CAPACITY);
        if property.is_null() {
            let allocation = allocate(0x1C);
            property = if allocation.is_null() {
                allocation
            } else {
                construct_property(allocation, 0)
            };
            vcall!(property, 4 => fn() -> u32);
            let variant = vcall!(property, 0x14 => fn() -> *mut u8);
            let initial = [0u16; 12];
            vcall!(variant, 0x188 => fn(*const u16, u32) -> u32, initial.as_ptr(), 12);
            vcall!(property, 0x10 => fn(u32) -> u32, CAPACITY);
            vcall!(holder, 0x3C => fn(*mut u8, u32) -> u32, property, 0);
            vcall!(property, 8 => fn() -> u32);
        }
        let variant = vcall!(property, 0x14 => fn() -> *mut u8);
        let kind_result = virtual_registers(variant, 0x0C, &[], variant as u32, word(variant, 0));
        let kind = kind_result.eax;
        let mut eax = kind;
        if kind == 0x8004 {
            let result = virtual_registers(variant, 0x18C, &[], word(variant, 0), kind_result.edx);
            let cells = result.eax as *mut u16;
            if value < 0xFFFF {
                cells.wrapping_add(index as usize).write_unaligned(value as u16);
                out.write(result);
                return;
            }
            let mut wide = [0u32; 12];
            for (i, dest) in wide.iter_mut().enumerate() {
                *dest = u32::from(cells.add(i).read_unaligned());
            }
            eax = vcall!(variant, 0x198 => fn(*const u32, u32) -> u32, wide.as_ptr(), 12);
        }
        let result = virtual_registers(variant, 0x19C, &[], eax, word(variant, 0));
        (result.eax as *mut u32)
            .wrapping_add(index as usize)
            .write_unaligned(value);
        out.write(result);
    }
}

port! {
    /// Stdcall wrapper for the register-based capacity writer; ECX is unused.
    #[port(address = 0x0070_DF70, owner = "traffic", name = "cSC4TrafficSimulator::SetMaxTripCapacity")]
    pub unsafe extern "stdcall" fn fc4p_0070df70(holder: *mut u8, index: u32, value: u32) -> u64 {
        let target = crate::abi::target(0x0070_DE70);
        let result = unsafe { call_set(target, holder, index, value) };
        (u64::from(result.edx) << 32) | u64::from(result.eax)
    }
}

unsafe fn call_set(target: usize, holder: *mut u8, index: u32, value: u32) -> Registers {
    let mut frame = [target as u32, holder as u32, value, index, 0, 0, 0];
    // SAFETY: ESI holds the frame across the register-based call. All callee-saved registers are restored.
    unsafe {
        core::arch::asm!("push esi", "push edi", "push ebx", "mov esi, ecx",
            "mov edi, [esi + 4]", "mov ebx, [esi + 8]", "push [esi + 12]", "call dword ptr [esi]", "add esp, 4",
            "mov [esi + 16], eax", "mov [esi + 20], ecx", "mov [esi + 24], edx",
            "pop ebx", "pop edi", "pop esi", in("ecx") frame.as_mut_ptr(), clobber_abi("C"));
    }
    Registers {
        eax: frame[4],
        ecx: frame[5],
        edx: frame[6],
    }
}

// Each adapter is explicit so the exported port module remains available to the table.
port! {
    /// Reads maximum capacity at index zero through three.
    #[port(address = 0x0070_DF90, owner = "traffic", name = "cSC4TrafficSimulator::GetMaxTripCapacity", naked)]
    pub unsafe extern "C" fn fc4p_0070df90() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp", "push [esp + 20]", "push [esp + 20]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 8", sym maximum);
    }
}
port! {
    /// Reads remaining trip capacity at index four through seven.
    #[port(address = 0x0070_DFF0, owner = "traffic", name = "GetTripCapacity", naked)]
    pub unsafe extern "C" fn fc4p_0070dff0() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp", "push [esp + 20]", "push [esp + 20]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 8", sym available);
    }
}
port! {
    /// Reads arrived traffic at index eight through eleven.
    #[port(address = 0x0070_E050, owner = "traffic", name = "cSC4TrafficSimulator::GetTrafficArrived", naked)]
    pub unsafe extern "C" fn fc4p_0070e050() {
        core::arch::naked_asm!("sub esp, 12", "mov [esp + 8], edx", "mov edx, esp", "push [esp + 20]", "push [esp + 20]", "call {}",
            "mov eax, [esp]", "mov ecx, [esp + 4]", "mov edx, [esp + 8]", "add esp, 12", "ret 8", sym arrived);
    }
}
unsafe extern "fastcall" fn maximum(_this: *mut u8, out: *mut Registers, holder: *mut u8, index: u32) {
    unsafe {
        read_capacity(out, holder, index, 0);
    }
}
unsafe extern "fastcall" fn available(_this: *mut u8, out: *mut Registers, holder: *mut u8, index: u32) {
    unsafe {
        read_capacity(out, holder, index, 4);
    }
}
unsafe extern "fastcall" fn arrived(_this: *mut u8, out: *mut Registers, holder: *mut u8, index: u32) {
    unsafe {
        read_capacity(out, holder, index, 8);
    }
}
unsafe fn read_capacity(out: *mut Registers, holder: *mut u8, index: u32, offset: u32) {
    unsafe {
        // The first call leaves EDX and ECX observable when the property is absent.
        let mut result = virtual_registers(holder, 0x24, &[CAPACITY], word(holder, 0), (*out).edx);
        if result.eax != 0 {
            let property = result.eax as *mut u8;
            let variant = vcall!(property, 0x14 => fn() -> *mut u8);
            let kind = vcall!(variant, 0x0C => fn() -> u32);
            let slot = if kind == 0x8004 { 0x18C } else { 0x19C };
            result = virtual_registers(variant, slot, &[], kind, word(variant, 0));
            let cells = result.eax as *mut u8;
            let i = index.wrapping_add(offset);
            result.eax = if kind == 0x8004 {
                u32::from(
                    cells
                        .wrapping_add(i.wrapping_mul(2) as usize)
                        .cast::<u16>()
                        .read_unaligned(),
                )
            } else {
                word(cells, i.wrapping_mul(4) as usize)
            };
            if offset == 0 {
                result.ecx = index;
            }
        }
        out.write(result);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_0070de70::PORT,
    fc4p_0070df70::PORT,
    fc4p_0070df90::PORT,
    fc4p_0070dff0::PORT,
    fc4p_0070e050::PORT,
];

#[cfg(test)]
mod tests;
