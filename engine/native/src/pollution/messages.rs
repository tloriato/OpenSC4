// SPDX-License-Identifier: GPL-3.0-or-later
//! Daily and monthly notifications, occupant changes and view visibility.

use super::*;

game_fn! { #[allow(dead_code)] fn daily(this: *mut u8) -> u64 = "thiscall" 0x006E_7FD0; }
game_fn! { #[allow(dead_code)] fn monthly(this: *mut u8) -> u64 = "thiscall" 0x006E_7B60; }
game_fn! { #[allow(dead_code)] fn begin(this: *mut u8) -> u64 = "thiscall" 0x006E_44B0; }
game_fn! { #[allow(dead_code)] fn visuals(this: *mut u8) -> u64 = "thiscall" 0x006E_63C0; }
game_fn! { #[allow(dead_code)] fn tuning(this: *mut u8) -> u64 = "thiscall" 0x006E_5B10; }
game_fn! { #[allow(dead_code)] fn occupant_changed(this: *mut u8, occupant: u32, insert: u32) -> u64 = "thiscall" 0x006E_4680; }

/// A message getter can leave part of ECX in the following boolean stack argument.
unsafe fn getter(message: *mut u8, slot: usize, mut eax: u32, mut edx: u32) -> (u32, u32, u32) {
    let mut ecx = message as u32;
    // SAFETY: message getters take no stack arguments and return integer words, with no x87 result.
    unsafe {
        core::arch::asm!(
            "call {target}", target = in(reg) vslot(message, slot),
            inlateout("eax") eax, inlateout("edx") edx, inlateout("ecx") ecx,
            clobber_abi("C"),
        );
    }
    (eax, edx, ecx)
}

unsafe fn dispatch(address: u32, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: each branch uses the original entry point and its original stack words.
    unsafe { call_raw(&mut call) };
    registers(call.eax, call.edx)
}

port! {
    /// Handles every message successfully. Only AL is replaced in the last result; EDX is retained.
    /// The message-target interface is four bytes after the primary interface. View changes compare
    /// the complete stored visibility byte and write it only when its value changes.
    #[port(address = 0x006E_8E20, owner = "pollution", name = "cSC4PollutionSimulator::DoMessage")]
    pub unsafe extern "fastcall" fn fc4p_006e8e20(this: *mut u8, entry_edx: u32, message: *mut u8) -> u64 {
        // SAFETY: the game supplies the message-target interface, message and their live services.
        unsafe {
            let primary = this.wrapping_sub(4);
            let sim = PollutionSim::from_ptr(primary);
            let (eax, edx, _) = getter(message, 0x10, table(message), entry_edx);
            let mut result = registers(eax, edx);
            match eax {
                0x6695_6814 => result = dispatch(0x006E_7FD0, primary, eax, edx, &[]),
                0x6695_6810 => result = dispatch(0x006E_44B0, primary, eax, edx, &[]),
                0x6695_6816 => result = dispatch(0x006E_7B60, primary, eax, edx, &[]),
                0x26D3_1EC1 => result = dispatch(0x006E_63C0, primary, eax, edx, &[]),
                0xCA1B_151C => result = dispatch(0x006E_5B10, primary, eax, edx, &[]),
                0x99EF_1142 | 0x99EF_1143 => {
                    let (occupant, edx, ecx) = getter(message, 0x28, eax, table(message));
                    let insert = (ecx & !0xFF) | u32::from(eax == 0x99EF_1142);
                    result = dispatch(0x006E_4680, primary, occupant, edx, &[occupant, insert]);
                }
                0xC6CD_BF1A => {
                    let (pointer, _, _) = getter(message, 0x3C, eax, table(message));
                    let level = read_unaligned(pointer as *const u32);
                    let visible = u8::from(level == 1);
                    let eax = (pointer & !0xFF) | u32::from(visible);
                    result = registers(eax, level);
                    if sim.effects_visible() != visible {
                        sim.set_effects_visible(visible);
                        result = dispatch(0x006E_63C0, primary, eax, level, &[]);
                    }
                }
                _ => {}
            }
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Returns the complete average-air word, preserving EDX and every other field.
    #[port(address = 0x0076_24C0, owner = "pollution", name = "cSC4PollutionSimulator::GetAverageAirValue", naked)]
    pub unsafe extern "C" fn fc4p_007624c0() {
        core::arch::naked_asm!("mov eax, dword ptr [ecx + 0x100]", "ret");
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e8e20::PORT, fc4p_007624c0::PORT];

#[cfg(test)]
mod tests;
