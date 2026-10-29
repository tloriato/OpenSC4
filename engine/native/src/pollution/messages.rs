// SPDX-License-Identifier: GPL-3.0-or-later
//! Daily and monthly notifications, occupant changes and view visibility.

use super::*;

game_fn! { #[allow(dead_code)] fn daily(this: *mut u8) -> u64 = "thiscall" 0x006E_7FD0; }
game_fn! { #[allow(dead_code)] fn monthly(this: *mut u8) -> u64 = "thiscall" 0x006E_7B60; }
game_fn! { #[allow(dead_code)] fn begin(this: *mut u8) -> u64 = "thiscall" 0x006E_44B0; }
game_fn! { #[allow(dead_code)] fn visuals(this: *mut u8) -> u64 = "thiscall" 0x006E_63C0; }
game_fn! { #[allow(dead_code)] fn tuning(this: *mut u8) -> u64 = "thiscall" 0x006E_5B10; }
game_fn! { #[allow(dead_code)] fn occupant_changed(this: *mut u8, occupant: u32, insert: u32) -> u64 = "thiscall" 0x006E_4680; }

#[cfg(test)]
unsafe fn dispatch(address: u32, this: *mut u8, eax: u32, edx: u32, args: &[u32]) -> u64 {
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(address), this as u32, args);
    call.eax = eax;
    call.edx = edx;
    // SAFETY: each branch uses the original entry point and its original stack words.
    unsafe { call_raw(&mut call) };
    registers(call.eax, call.edx)
}

// Keep the call on the original frame: visual updates expose a stack address in
// their partial-register return. Test adapters still resolve thread-local fakes.
#[cfg(not(test))]
static HANDLERS: [usize; 6] = [
    0x006E_7FD0,
    0x006E_44B0,
    0x006E_7B60,
    0x006E_63C0,
    0x006E_5B10,
    0x006E_4680,
];
#[cfg(test)]
macro_rules! handler_adapter {
    ($name:ident, $address:literal) => {
        unsafe extern "fastcall" fn $name(this: *mut u8, edx: u32) -> u64 {
            unsafe { dispatch($address, this, 0, edx, &[]) }
        }
    };
}
#[cfg(test)]
handler_adapter!(call_daily, 0x006E_7FD0);
#[cfg(test)]
handler_adapter!(call_begin, 0x006E_44B0);
#[cfg(test)]
handler_adapter!(call_monthly, 0x006E_7B60);
#[cfg(test)]
handler_adapter!(call_visuals, 0x006E_63C0);
#[cfg(test)]
handler_adapter!(call_tuning, 0x006E_5B10);
#[cfg(test)]
unsafe extern "fastcall" fn call_changed(this: *mut u8, edx: u32, occupant: u32, insert: u32) -> u64 {
    unsafe { dispatch(0x006E_4680, this, occupant, edx, &[occupant, insert]) }
}
#[cfg(test)]
type Handler = unsafe extern "fastcall" fn(*mut u8, u32) -> u64;
#[cfg(test)]
type ChangedHandler = unsafe extern "fastcall" fn(*mut u8, u32, u32, u32) -> u64;
#[cfg(test)]
type ErasedHandler = unsafe extern "fastcall" fn();
#[cfg(test)]
// SAFETY: assembly loads these addresses and supplies each handler's actual arguments.
static HANDLERS: [ErasedHandler; 6] = unsafe {
    [
        core::mem::transmute::<Handler, ErasedHandler>(call_daily),
        core::mem::transmute::<Handler, ErasedHandler>(call_begin),
        core::mem::transmute::<Handler, ErasedHandler>(call_monthly),
        core::mem::transmute::<Handler, ErasedHandler>(call_visuals),
        core::mem::transmute::<Handler, ErasedHandler>(call_tuning),
        core::mem::transmute::<ChangedHandler, ErasedHandler>(call_changed),
    ]
};

port! {
    /// Handles every message successfully. Only AL is replaced in the last result; EDX is retained.
    /// The message-target interface is four bytes after the primary interface. View changes compare
    /// the complete stored visibility byte and write it only when its value changes.
    #[port(address = 0x006E_8E20, owner = "pollution", name = "cSC4PollutionSimulator::DoMessage", naked)]
    pub unsafe extern "fastcall" fn fc4p_006e8e20(this: *mut u8, entry_edx: u32, message: *mut u8) -> u64 {
        core::arch::naked_asm!(
            "push ebx", "push esi", "push edi", "mov edi, [esp + 16]", "mov eax, [edi]",
            "mov ebx, ecx", "mov ecx, edi", "call dword ptr [eax + 0x10]", "mov esi, eax",
            "cmp esi, 0x66956816", "ja 5f", "je 4f", "sub esi, 0x26d31ec1", "je 8f",
            "sub esi, 0x3fc2494f", "je 3f", "sub esi, 4", "jne 20f",
            "lea ecx, [ebx - 4]", "call dword ptr [{handlers}]", "jmp 20f",
            "3:", "lea ecx, [ebx - 4]", "call dword ptr [{handlers} + 4]", "jmp 20f",
            "4:", "lea ecx, [ebx - 4]", "call dword ptr [{handlers} + 8]", "jmp 20f",
            "5:", "cmp esi, 0xc6cdbf1a", "ja 9f", "je 7f",
            "cmp esi, 0x99ef1142", "jb 20f", "cmp esi, 0x99ef1143", "ja 20f",
            "mov edx, [edi]", "mov ecx, edi", "call dword ptr [edx + 0x28]",
            "cmp esi, 0x99ef1142", "sete cl", "push ecx", "push eax", "lea ecx, [ebx - 4]",
            "call dword ptr [{handlers} + 20]", "jmp 20f",
            "7:", "mov edx, [edi]", "mov ecx, edi", "call dword ptr [edx + 0x3c]",
            "mov edx, [eax]", "mov cl, [ebx + 0x208]", "cmp edx, 1", "sete al",
            "cmp cl, al", "je 20f", "mov [ebx + 0x208], al",
            "8:", "lea ecx, [ebx - 4]", "call dword ptr [{handlers} + 12]", "jmp 20f",
            "9:", "cmp esi, 0xca1b151c", "jne 20f", "lea ecx, [ebx - 4]",
            "call dword ptr [{handlers} + 16]",
            "20:", "pop edi", "pop esi", "mov al, 1", "pop ebx", "ret 4",
            handlers = sym HANDLERS,
        );
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
