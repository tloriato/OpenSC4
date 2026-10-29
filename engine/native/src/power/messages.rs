// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Power notifications through the message-target interface.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};

layout! {
    /// Message dispatch uses offsets from the primary interface.
    struct MessageSim {
        0x14 => enabled / set_enabled: u8,
        0x18 => city / set_city: *mut u8,
        0x298 => plants / set_plants: *mut u8,
        0x29C => poles_begin / set_poles_begin: *mut u8,
        0x2A0 => poles_end / set_poles_end: *mut u8,
        0x2B8 => pole_count / set_pole_count: u32,
        0x338 => recompute / set_recompute: u8,
        0x34C => state / set_state: u32,
    }
}

// These calls use RawCall below so even an otherwise unused EDX result survives dispatch.
game_fn! { #[allow(dead_code)] fn idle(this: *mut u8, budget: u32) -> u64 = "thiscall" 0x006E_E960; }
game_fn! { #[allow(dead_code)] fn monthly(this: *mut u8) -> u64 = "thiscall" 0x006E_C570; }
game_fn! { #[allow(dead_code)] fn tuning(this: *mut u8) -> u64 = "thiscall" 0x006E_D1C0; }
game_fn! { #[allow(dead_code)] fn inserted(this: *mut u8, occupant: u32) -> u64 = "thiscall" 0x006E_CC50; }
game_fn! { #[allow(dead_code)] fn removed(this: *mut u8, occupant: u32) -> u64 = "thiscall" 0x006E_BF60; }
game_fn! { #[allow(dead_code)] fn costs(this: *mut u8) -> u64 = "thiscall" 0x006E_B490; }
game_fn! { #[allow(dead_code)] fn cell_changed(this: *mut u8, x0: u32, z0: u32, x1: u32, z1: u32) -> u64 = "thiscall" 0x006E_B350; }
game_fn! { #[allow(dead_code)] fn terrain_changed(this: *mut u8, low: *mut u32, high: *mut u32) -> u64 = "thiscall" 0x006E_DDA0; }
game_fn! { #[allow(dead_code)] fn erase_pole(this: *mut u8, position: *mut u8) -> u64 = "thiscall" 0x0064_5A60; }
game_fn! { #[allow(dead_code)] fn network_manager() -> u64 = "C" 0x0060_2680; }
game_fn! { #[allow(dead_code)] fn list_new(this: *mut u8, allocator: *mut u8) -> u64 = "thiscall" 0x004F_5A00; }
game_fn! { #[allow(dead_code)] fn ref_new(this: *mut u8, object: *mut u8) -> u64 = "thiscall" 0x004F_20C0; }
game_fn! { #[allow(dead_code)] fn list_push(this: *mut u8, reference: *mut u8) -> u64 = "thiscall" 0x007E_98C0; }
game_fn! { #[allow(dead_code)] fn list_delete(this: *mut u8) -> u64 = "thiscall" 0x0042_7670; }
game_fn! { #[allow(dead_code)] fn ref_out(this: *mut u8) -> u64 = "thiscall" 0x004F_20A0; }
game_fn! { #[allow(dead_code)] fn property(object: *mut u8, id: u32, out: *mut u32) -> u64 = "C" 0x005F_D480; }

game_data! { static demolition: *mut u8 = 0x00B4_3D00; }
game_data! { static lots: *mut u8 = 0x00B4_3D08; }
game_data! { static tools: *mut u8 = 0x00B4_3DC4; }

pub(super) struct Registers {
    pub(super) eax: u32,
    pub(super) edx: u32,
}
impl Registers {
    pub(super) unsafe fn call(&mut self, target: usize, this: *mut u8, args: &[u32], conv: Conv) {
        let mut c = RawCall::new(conv, target, this as u32, args);
        c.eax = self.eax;
        c.edx = self.edx;
        unsafe { call_raw(&mut c) };
        self.eax = c.eax;
        self.edx = c.edx;
    }
    pub(super) unsafe fn game(&mut self, address: u32, this: *mut u8, args: &[u32], conv: Conv) {
        unsafe { self.call(crate::abi::target(address), this, args, conv) };
    }
    pub(super) unsafe fn virtual_eax(&mut self, this: *mut u8, offset: usize, args: &[u32]) {
        self.eax = unsafe { word(this, 0) };
        unsafe { self.call(vslot(this, offset), this, args, Conv::Thiscall) };
    }
    pub(super) unsafe fn virtual_edx(&mut self, this: *mut u8, offset: usize, args: &[u32]) {
        self.edx = unsafe { word(this, 0) };
        unsafe { self.call(vslot(this, offset), this, args, Conv::Thiscall) };
    }
    fn result(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from((self.eax & !255) | 1)
    }
}

pub(super) unsafe fn word(p: *mut u8, offset: usize) -> u32 {
    unsafe { p.add(offset).cast::<u32>().read_unaligned() }
}
pub(super) unsafe fn put(p: *mut u8, offset: usize, value: u32) {
    unsafe { p.add(offset).cast::<u32>().write_unaligned(value) };
}

port! {
    /// Dispatch power notifications, retaining the upper return bytes of the last query or helper.
    /// The scratch words include the message argument, which doubles as a reference/list holder.
    #[port(address = 0x006E_FB70, owner = "power", name = "cSC4PowerSimulator::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_006efb70() {
        core::arch::naked_asm!(
            "sub esp, 0x18", "push ebx", "push ebp", "push esi", "push edi",
            "push edx", "lea edx, [esp + 4]", "call {body}",
            "pop edi", "pop esi", "pop ebp", "pop ebx", "add esp, 0x18", "ret 4",
            body = sym dispatch,
        );
    }
}

unsafe extern "fastcall" fn dispatch(interface: *mut u8, frame: *mut u8, entry_edx: u32) -> u64 {
    unsafe {
        let this = interface.wrapping_sub(4);
        let sim = MessageSim::from_ptr(this);
        let message = word(frame, 0x2C) as *mut u8;
        let mut r = Registers { eax: 0, edx: entry_edx };
        r.virtual_eax(message, 0x10, &[]);
        let id = r.eax;
        match id {
            0x6AC2_84F3 => sim.set_enabled(1),
            0x6695_6814 => r.game(0x006E_E960, this, &[5], Conv::Thiscall),
            0x6695_6816 => {
                r.eax = 0;
                r.game(0x006E_C570, this, &[], Conv::Thiscall);
            }
            0x6695_681A => {
                r.virtual_eax(message, 0x4C, &[]);
                r.game(0x006E_E960, this, &[r.eax], Conv::Thiscall);
            }
            0xCA1B_151C => r.game(0x006E_D1C0, this, &[], Conv::Thiscall),
            0x99EF_1142 | 0x99EF_1143 => {
                if id == 0x99EF_1142 {
                    r.virtual_eax(message, 0x28, &[]);
                    r.game(0x006E_CC50, this, &[r.eax], Conv::Thiscall);
                } else {
                    r.virtual_edx(message, 0x28, &[]);
                    r.game(0x006E_BF60, this, &[r.eax], Conv::Thiscall);
                }
            }
            0x278E_6003 => {
                r.virtual_edx(message, 0x60, &[]);
                if r.eax != 4 && r.eax != 5 {
                    r.virtual_eax(message, 0x24, &[]);
                    let x = r.eax;
                    r.virtual_edx(message, 0x38, &[]);
                    let z = r.eax;
                    put(frame, 0x10, (x >> 16).wrapping_sub(1));
                    put(frame, 0x14, (z >> 16).wrapping_sub(1));
                    put(frame, 0x18, x & 0xFFFF);
                    put(frame, 0x1C, z & 0xFFFF);
                    r.virtual_eax(sim.city(), 0x174, &[]);
                    let max_x = r.eax.wrapping_sub(1) as i32;
                    r.virtual_edx(sim.city(), 0x178, &[]);
                    let max_z = r.eax.wrapping_sub(1) as i32;
                    r.edx = word(frame, 0x10);
                    r.eax = max_z as u32;
                    put(frame, 0x10, (word(frame, 0x10) as i32).max(0) as u32);
                    put(frame, 0x14, (word(frame, 0x14) as i32).max(0) as u32);
                    put(frame, 0x18, (word(frame, 0x18) as i32).min(max_x) as u32);
                    put(frame, 0x1C, (word(frame, 0x1C) as i32).min(max_z) as u32);
                    r.eax = frame.add(0x18) as u32;
                    r.game(0x006E_DDA0, this, &[frame.add(0x10) as u32, r.eax], Conv::Thiscall);
                }
            }
            0x26D3_1EC1 => {
                let mut cursor = sim.poles_begin();
                while cursor != sim.poles_end() {
                    let pole = word(cursor, 0) as *mut u8;
                    r.eax = frame.add(0x2C) as u32;
                    put(frame, 0x2C, 0);
                    r.virtual_edx(pole, 0, &[0x26BB_767E, r.eax]);
                    let mut erase = false;
                    if r.eax as u8 != 0 {
                        r.virtual_edx(word(frame, 0x2C) as *mut u8, 0x80, &[1]);
                        erase = r.eax as u8 == 0;
                    }
                    if erase {
                        r.virtual_eax(pole, 8, &[]);
                        r.game(0x0064_5A60, this.add(0x29C), &[cursor as u32], Conv::Thiscall);
                        cursor = r.eax as *mut u8;
                        sim.set_pole_count(sim.pole_count().wrapping_sub(1));
                    } else {
                        cursor = cursor.wrapping_add(4);
                    }
                    let reference = word(frame, 0x2C) as *mut u8;
                    if !reference.is_null() {
                        r.virtual_edx(reference, 8, &[]);
                    }
                }
                r.game(0x006E_B490, this, &[], Conv::Thiscall);
                if r.eax as u8 != 0 {
                    sim.set_recompute(1);
                }
            }
            0x0769_9455 => {
                r.virtual_eax(message, 0x3C, &[]);
                let rect = r.eax as *mut u8;
                r.game(0x0060_2680, core::ptr::null_mut(), &[], Conv::Cdecl);
                let manager = r.eax as *mut u8;
                r.virtual_edx(manager, 0x14, &[5, 0]);
                let tool = r.eax as *mut u8;
                r.virtual_eax(tool, 4, &[]);
                r.virtual_edx(tool, 0x0C, &[]);
                r.eax = word(rect, 0);
                r.edx = word(rect, 4);
                let bounds = [r.eax, r.edx, word(rect, 8), word(rect, 12)];
                r.game(0x006E_B350, this, &bounds, Conv::Thiscall);
                // The tool's vtable is captured before querying message data, whose callback may replace it.
                r.edx = word(message, 0);
                let tool_slot = vslot(tool, 0x28);
                r.call(vslot(message, 0x4C), message, &[], Conv::Thiscall);
                let data = r.eax;
                r.edx = word(rect, 12);
                r.eax = frame.wrapping_sub(0x14) as u32;
                r.call(
                    tool_slot,
                    tool,
                    &[word(rect, 0), word(rect, 4), word(rect, 8), r.edx, data],
                    Conv::Thiscall,
                );
                r.virtual_eax(tool, 0x10, &[]);
                r.virtual_edx(tool, 8, &[]);
            }
            0xC2C0_9554 => {
                r.virtual_edx(message, 0x38, &[]);
                if r.eax == 0xE0D0_722E {
                    r.virtual_eax(message, 0x4C, &[]);
                    if r.eax != 0 {
                        nuclear_free_zone(sim, frame, &mut r);
                    }
                }
            }
            0xEA41_8D8F => {
                r.virtual_edx(message, 0x24, &[]);
                if r.eax == 0 {
                    put(frame, 0x2C, 0);
                    r.virtual_eax(message, 0x80, &[]);
                    let unknown = r.eax as *mut u8;
                    if !unknown.is_null() {
                        let slot = vslot(unknown, 0);
                        r.game(0x004F_20A0, frame.add(0x2C), &[], Conv::Thiscall);
                        r.call(slot, unknown, &[0x0773_FEF1, r.eax], Conv::Thiscall);
                        if r.eax as u8 != 0 {
                            r.virtual_edx(word(frame, 0x2C) as *mut u8, 0x20, &[]);
                            let buy = r.eax == 0x4773_FFE1;
                            if !buy {
                                r.virtual_eax(word(frame, 0x2C) as *mut u8, 0x20, &[]);
                            }
                            if buy || r.eax == 0x4773_FFE0 {
                                sim.set_state(4);
                                sim.set_recompute(1);
                            }
                        }
                    }
                    let reference = word(frame, 0x2C) as *mut u8;
                    if !reference.is_null() {
                        r.virtual_edx(reference, 8, &[]);
                    }
                }
            }
            0xE8BD_A2E7 => {
                r.virtual_eax(message, 0x80, &[]);
                if r.eax != 0 {
                    r.virtual_edx(message, 0x80, &[]);
                    let unknown = r.eax as *mut u8;
                    r.virtual_edx(unknown, 0, &[0xE818_FCED, frame.add(0x2C) as u32]);
                    if r.eax as u8 != 0 {
                        r.virtual_edx(word(frame, 0x2C) as *mut u8, 0x84, &[]);
                        if r.eax == 1 {
                            r.virtual_eax(word(frame, 0x2C) as *mut u8, 0x38, &[frame.add(0x18) as u32]);
                            // Signed sums wrap before division. Negative sums are rounded upward after adding one.
                            let sum_x = word(frame, 0x18).wrapping_add(word(frame, 0x20)) as i32;
                            let sum_z = word(frame, 0x1C).wrapping_add(word(frame, 0x24)) as i32;
                            let x = sum_x.wrapping_sub(sum_x >> 31) >> 1;
                            let z = sum_z.wrapping_sub(sum_z >> 31) >> 1;
                            r.eax = sum_z.wrapping_sub(sum_z >> 31) as u32;
                            r.virtual_edx(this, 0x94, &[x as u32, z as u32]);
                            if r.eax as u8 != 0 {
                                r.virtual_eax(*lots(), 0x18, &[x as u32, z as u32, 0]);
                                if r.eax != 0 {
                                    r.virtual_edx(r.eax as *mut u8, 0x10C, &[1]);
                                }
                            }
                        }
                        r.virtual_eax(word(frame, 0x2C) as *mut u8, 8, &[]);
                    }
                }
            }
            _ => {
                if id > 0x6695_6814 && id < 0x6AC2_84F3 {
                    r.eax = id.wrapping_sub(0x6695_6816).wrapping_sub(4);
                }
            }
        }
        r.result()
    }
}

unsafe fn nuclear_free_zone(sim: MessageSim, frame: *mut u8, r: &mut Registers) {
    unsafe {
        r.game(0x004F_5A00, frame.add(0x2C), &[frame.add(0x2C) as u32], Conv::Thiscall);
        let head = sim.plants();
        let mut node = word(head, 0) as *mut u8;
        while node != head {
            r.virtual_edx(word(node, 12) as *mut u8, 0x18, &[]);
            r.game(
                0x005F_D480,
                core::ptr::null_mut(),
                &[r.eax, 0x2781_2853, frame.add(0x18) as u32],
                Conv::Cdecl,
            );
            if r.eax as u8 != 0 && word(frame, 0x18) == 5 {
                r.eax = word(node, 12);
                r.virtual_edx(*lots(), 0x38, &[r.eax]);
                r.game(0x004F_20C0, frame.add(0x10), &[r.eax], Conv::Thiscall);
                r.game(0x007E_98C0, frame.add(0x2C), &[frame.add(0x10) as u32], Conv::Thiscall);
                let reference = word(frame, 0x10) as *mut u8;
                if !reference.is_null() {
                    r.virtual_edx(reference, 8, &[]);
                }
            }
            node = word(node, 0) as *mut u8;
        }
        r.edx = frame.add(0x2C) as u32;
        r.virtual_eax(*demolition(), 0x1C, &[1, frame.add(0x2C) as u32, 0, 1, 0, 0, 0, 0]);
        r.virtual_eax(*tools(), 0x60, &[]);
        if r.eax != 0 {
            r.virtual_edx(r.eax as *mut u8, 0x14, &[]);
            if r.eax == 0x88F1_54FB {
                r.virtual_eax(*tools(), 0x68, &[0]);
            }
        }
        r.game(0x0042_7670, frame.add(0x2C), &[], Conv::Thiscall);
    }
}

pub const PORTS: &[Port] = &[fc4p_006efb70::PORT];

#[cfg(test)]
mod tests;
