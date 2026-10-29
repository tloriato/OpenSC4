// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Occupant queries, exemplar acquisition, and adjacent-cell probes.

use super::*;

game_data! { static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_fn! { fn get_exemplar(this: *mut u8, instance: u32, group: u32) -> *mut u8 = "thiscall" 0x0062_44B0; }
game_fn! { fn add_model(this: *mut u8, exemplar: *mut u8, a: u32, b: u32) = "thiscall" 0x0061_D120; }
game_fn! {
    #[allow(dead_code)]
    fn lookup_cell(this: *mut u8, key: u32) -> u64 = "thiscall" 0x0063_3220;
}

#[derive(Clone, Copy)]
pub(super) struct Registers {
    pub eax: u32,
    pub ecx: u32,
    pub edx: u32,
}
// These callers expose ECX after a virtual call as well as EDX:EAX. Save all three before Rust resumes.
pub(super) unsafe fn call(target: usize, args: &[u32], state: Registers) -> Registers {
    let mut c = crate::abi::RawCall::new(crate::abi::Conv::Thiscall, target, state.ecx, args);
    c.eax = state.eax;
    c.edx = state.edx;
    unsafe {
        core::arch::asm!(
            "mov ecx, [edi + 20]", "mov eax, [edi + 16]",
            "2:", "test ecx, ecx", "jz 3f", "dec ecx", "push dword ptr [eax + ecx*4]", "jmp 2b",
            "3:", "mov eax, [edi + 4]", "mov ecx, [edi + 8]", "mov edx, [edi + 12]",
            "call dword ptr [edi]", "mov [edi + 4], eax", "mov [edi + 8], ecx", "mov [edi + 12], edx",
            in("edi") &mut c, clobber_abi("C"),
        );
    }
    Registers {
        eax: c.eax,
        ecx: c.ecx,
        edx: c.edx,
    }
}
pub(super) unsafe fn virtual_call(this: *mut u8, slot: usize, args: &[u32], eax: u32, edx: u32) -> Registers {
    unsafe {
        call(
            crate::abi::vslot(this, slot),
            args,
            Registers {
                eax,
                ecx: this as u32,
                edx,
            },
        )
    }
}
register_port!(
    0x0062_4480,
    fc4p_00624480,
    "RememberMatchingOccupantId",
    remember_id,
    "ret 4"
);
unsafe extern "fastcall" fn remember_id(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let object = frame.add(3).read() as *mut u8;
        let mut r = virtual_call(object, 0x10, &[], read(object, 0), frame.read());
        if r.eax == 0xA6B7_9602 {
            r = virtual_call(object, 0x3C, &[], r.eax, read(object, 0));
            r.eax = read(r.eax as *mut u8, 0);
            write(this, 0xF8, r.eax);
        }
        registers((r.eax & !255) | 1, r.edx)
    }
}
port! {
    /// Acquire an exemplar using the supplied group, or the active type's group when it is zero. Only
    /// the low success byte controls the returned pointer; keep the resource service's output unchanged.
    #[port(address = 0x0062_44B0, owner = "networktool", name = "cSC4NetworkTool::GetOccupantExemplar", naked)]
    pub unsafe extern "C" fn fc4p_006244b0() {
        core::arch::naked_asm!(
            "sub esp, 12", "mov edx, esp", "call {body}", "add esp, 12", "ret 8",
            body = sym occupant_exemplar,
        );
    }
}
unsafe extern "fastcall" fn occupant_exemplar(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let instance = frame.add(4).read();
        let group = frame.add(5).read();
        let selected = if group == 0 {
            read(NetworkTool::from_ptr(this).type_info(), 0xC)
        } else {
            group
        };
        frame.write(0x6534_284A);
        frame.add(1).write(selected);
        frame.add(2).write(instance);
        let manager = *resource_manager();
        let result = virtual_call(
            manager,
            0xC,
            &[frame as u32, 0xA521_60F5, frame.add(5) as u32, 0, 0],
            read(manager, 0),
            frame as u32,
        );
        registers(if result.eax as u8 != 0 { frame.add(5).read() } else { 0 }, result.edx)
    }
}
register_port!(
    0x0062_4510,
    fc4p_00624510,
    "HasAdjacentNetworkAlongX",
    adjacent_x,
    "ret 12"
);
unsafe extern "fastcall" fn adjacent_x(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let x = frame.add(3).read();
        let z = frame.add(4).read();
        let mask = frame.add(5).read();
        let mut r = Registers {
            eax: frame.add(1).read(),
            ecx: this as u32,
            edx: frame.read(),
        };
        if x != 0 {
            r.eax = z.wrapping_shl(16);
            let key = r.eax.wrapping_add(x).wrapping_sub(1);
            r = call(crate::abi::target(0x0063_3220), &[key], r);
            if r.eax != 0 {
                r.eax = read::<u32>(r.eax as *mut u8, 8) & !mask;
                r.edx = !mask;
                if r.eax & 0x1FFF != 0 {
                    return registers((r.eax & !255) | 1, r.edx);
                }
            }
        }
        if x < NetworkTool::from_ptr(this).width().wrapping_sub(1) {
            let key = z.wrapping_shl(16).wrapping_add(x).wrapping_add(1);
            r.ecx = this as u32;
            r.edx = key;
            r = call(crate::abi::target(0x0063_3220), &[key], r);
            if r.eax != 0 {
                r.eax = read::<u32>(r.eax as *mut u8, 8) & !mask;
                if r.eax & 0x1FFF != 0 {
                    return registers((r.eax & !255) | 1, r.edx);
                }
            }
        }
        registers(r.eax & !255, r.edx)
    }
}
register_port!(
    0x0062_4590,
    fc4p_00624590,
    "HasAdjacentNetworkAlongZ",
    adjacent_z,
    "ret 12"
);
unsafe extern "fastcall" fn adjacent_z(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let x = frame.add(3).read();
        let z = frame.add(4).read();
        let mask = frame.add(5).read();
        let mut r = Registers {
            eax: frame.add(1).read(),
            ecx: this as u32,
            edx: frame.read(),
        };
        if z != 0 {
            let key = z.wrapping_sub(1).wrapping_shl(16).wrapping_add(x);
            r.eax = key;
            r = call(crate::abi::target(0x0063_3220), &[key], r);
            if r.eax != 0 {
                r.edx = read::<u32>(r.eax as *mut u8, 8) & !mask;
                if r.edx & 0x1FFF != 0 {
                    return registers((r.eax & !255) | 1, r.edx);
                }
            }
        }
        r.eax = NetworkTool::from_ptr(this).height().wrapping_sub(1);
        if z < r.eax {
            let key = z.wrapping_add(1).wrapping_shl(16).wrapping_add(x);
            r.ecx = this as u32;
            r = call(crate::abi::target(0x0063_3220), &[key], r);
            if r.eax != 0 && read::<u32>(r.eax as *mut u8, 8) & !mask & 0x1FFF != 0 {
                return registers((r.eax & !255) | 1, r.edx);
            }
        }
        registers(r.eax & !255, r.edx)
    }
}
port! {
    /// Query the model interface, acquire its exemplar, add the model and release both temporary
    /// references. A nonnull interface output is released even when QueryInterface reports failure.
    #[port(address = 0x0062_4860, owner = "networktool", name = "cSC4NetworkTool::SetExemplarForOccupant")]
    pub unsafe extern "thiscall" fn fc4p_00624860(this: *mut u8, occupant: *mut u8, instance: u32, a: u32, b: u32, group: u32) {
        unsafe {
            let mut interface = core::ptr::null_mut();
            if vcall!(occupant, 0 => fn(u32, *mut *mut u8) -> u32, 0x49C1_A034, &mut interface) as u8 != 0 {
                let exemplar = get_exemplar(this, instance, group);
                if !exemplar.is_null() {
                    add_model(interface, exemplar, a, b);
                    vcall!(exemplar, 8 => fn() -> u32);
                }
            }
            if !interface.is_null() { vcall!(interface, 8 => fn() -> u32); }
        }
    }
}
port! {
    /// Obtain an occupant for an in-bounds cell, query its network interface, then release the
    /// temporary occupant. Return the interface query's low byte and the last call's remaining bits.
    #[port(address = 0x0062_4B70, owner = "networktool", name = "cSC4NetworkTool::GetNetworkOccupant", naked)]
    pub unsafe extern "C" fn fc4p_00624b70() {
        core::arch::naked_asm!("push eax", "push edx", "mov edx, esp", "call {body}",
            "mov ecx, [esp + 4]", "add esp, 8", "ret 16", body = sym network_occupant)
    }
}
unsafe extern "fastcall" fn network_occupant(this: *mut u8, frame: *mut u32) -> u64 {
    unsafe {
        let x = frame.add(3).read();
        let z = frame.add(4).read();
        let flags = frame.add(5).read();
        let output = frame.add(6).read();
        let tool = NetworkTool::from_ptr(this);
        let mut r = Registers {
            eax: frame.add(1).read(),
            ecx: this as u32,
            edx: x,
        };
        if x < tool.width() {
            r.eax = z;
            if z < tool.height() {
                let manager: *mut u8 = read(this, 0x48);
                // The manager output is the original first argument word, subsequently read for Release.
                frame.add(3).write(0);
                r = virtual_call(
                    manager,
                    0x80,
                    &[frame.add(3) as u32, x, z, flags],
                    frame.add(3) as u32,
                    r.edx,
                );
                let query = r.eax as u8 != 0;
                r.ecx = frame.add(3).read();
                if query {
                    let occupant = r.ecx as *mut u8;
                    r = virtual_call(occupant, 0, &[0xA821_EF94, output], output, read(occupant, 0));
                    r.ecx = frame.add(3).read();
                    let answer = r.eax as u8;
                    if r.ecx != 0 {
                        let occupant = r.ecx as *mut u8;
                        r = virtual_call(occupant, 8, &[], r.eax, read(occupant, 0));
                    }
                    r.eax = (r.eax & !255) | u32::from(answer);
                    frame.add(1).write(r.ecx);
                    return registers(r.eax, r.edx);
                }
                if r.ecx != 0 {
                    let occupant = r.ecx as *mut u8;
                    r = virtual_call(occupant, 8, &[], read(occupant, 0), r.edx);
                }
            }
        }
        frame.add(1).write(r.ecx);
        registers(r.eax & !255, r.edx)
    }
}

pub const PORTS: &[Port] = &[
    fc4p_00624480::PORT,
    fc4p_006244b0::PORT,
    fc4p_00624510::PORT,
    fc4p_00624590::PORT,
    fc4p_00624860::PORT,
    fc4p_00624b70::PORT,
];
#[cfg(test)]
mod tests;
