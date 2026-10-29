// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Refreshing occupied lots after their terrain or configuration changes.

use super::*;

game_fn! {
    #[allow(dead_code)]
    fn update_altitudes(this: *mut u8, lot: *mut u8) -> u64 = "thiscall" 0x006C_91F0;
}
game_data! { static altitude_threshold: f32 = 0x00A8_821C; }

/// Compare an x87 value with a stored single without first quieting the stored operand.
fn compare_float(value: F80, operand: f32) -> u16 {
    let status: u16;
    unsafe {
        core::arch::asm!(
            "fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand,
            out("ax") status, out("st(0)") _,
        );
    }
    status
}

port! {
    /// Keeps the caller's argument words available as scratch and restores the saved ECX word.
    #[port(address = 0x006C_C880, owner = "lotdeveloper", name = "cSC4LotDeveloper::UpdateLotElements", naked)]
    pub unsafe extern "C" fn fc4p_006cc880() {
        core::arch::naked_asm!(
            "sub esp, 4", "lea eax, [esp + 8]", "push esp", "push eax", "call {}",
            "pop ecx", "ret 8", sym update_elements,
        );
    }
}

/// The lot word becomes the slope scratch value; the kind word may have its low byte replaced by the
/// selected foundation. The texture refresh word keeps the upper bytes of the developer pointer.
unsafe extern "fastcall" fn update_elements(
    this: *mut u8,
    entry_edx: u32,
    arguments: *mut u32,
    saved_ecx: *mut u32,
) -> u64 {
    unsafe {
        let lot = arguments.read_unaligned() as *mut u8;
        let mut refresh = this as u32;
        let mut call = virtual_call(lot, 0x84, &[], table(lot), entry_edx, false);
        if call.eax as i32 >= 2 {
            refresh &= !255;
            call = virtual_call(lot, 0x30, &[], call.eax, table(lot), false);
            if call.eax as u8 & 3 != 0 {
                call = virtual_call(this, 0xF4, &[lot as u32], table(this), call.edx, false);
                virtual_call(this, 0xF8, &[lot as u32], call.eax, table(this), false);
            } else if arguments.add(1).read_unaligned() != 5 {
                call = virtual_call(lot, 0x24, &[], table(lot), call.edx, false);
                let config = call.eax as *mut u8;
                if !config.is_null() {
                    call = virtual_call(config, 0x38, &[], call.eax, table(config), false);
                    arguments.cast::<u8>().write(call.eax as u8);
                    call = virtual_call(
                        this,
                        0x78,
                        &[lot as u32, arguments.read_unaligned()],
                        table(this),
                        call.edx,
                        true,
                    );
                    let slope = call.st0.to_f32();
                    arguments.write_unaligned(slope.to_bits());
                    call = virtual_call(config, 0x28, &[], call.eax, table(config), true);
                    let status = compare_float(call.st0, slope);
                    call.eax = (call.eax & !0xFFFF) | u32::from(status);
                    if (status >> 8) & 5 == 1 {
                        call = virtual_call(config, 0x6C, &[7], table(config), call.edx, false);
                        arguments.add(1).cast::<u8>().write(call.eax as u8);
                        call = virtual_call(
                            this,
                            0xD8,
                            &[lot as u32, config as u32, arguments.add(1).read_unaligned(), 0, 1],
                            arguments.add(1).read_unaligned(),
                            table(this),
                            false,
                        );
                        refresh |= 1;
                    }
                    let status = compare_float(F80::from_f32(slope).abs(), altitude_threshold().read());
                    call.eax = (call.eax & !0xFFFF) | u32::from(status);
                    if (status >> 8) & 0x41 == 0 {
                        call.target = crate::abi::target(0x006C_91F0);
                        call.ecx = this as u32;
                        let args = [lot as u32];
                        call.args = args.as_ptr();
                        call.count = 1;
                        call.take_st0 = 0;
                        call_raw(&mut call);
                    }
                }
            }
            call = virtual_call(this, 0xB4, &[lot as u32, refresh], refresh, table(this), false);
            call = virtual_call(lot, 0x50, &[], call.eax, table(lot), false);
        }
        saved_ecx.write_unaligned(refresh);
        call.eax = (call.eax & !255) | 1;
        result(call)
    }
}

pub const PORTS: &[Port] = &[fc4p_006cc880::PORT];
