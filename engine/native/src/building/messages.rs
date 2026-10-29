// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Message dispatch, including the caller's argument writes and the retained return-register bytes.

use crate::abi::{call_raw, target, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::ptr::{read_unaligned as read, write_unaligned as write};

// These calls use RawCall below because some exits retain both return registers, including bytes that
// a boolean callee leaves alone. Virtual calls likewise supply the table in its original register.
game_fn! { #[allow(dead_code)] fn monthly(this: *mut u8) -> u64 = "thiscall" 0x0069_6130; }
game_fn! { #[allow(dead_code)] fn idle(this: *mut u8, budget: u32) -> u64 = "thiscall" 0x0069_9EA0; }
game_fn! { #[allow(dead_code)] fn daily(this: *mut u8, day: u32) -> u64 = "thiscall" 0x0069_B6D0; }
game_fn! { #[allow(dead_code)] fn terrain_changed(this: *mut u8, rect: *const i32, kind: u32) -> u64 = "thiscall" 0x0069_8610; }
game_fn! { #[allow(dead_code)] fn building_changed(this: *mut u8, lot: *mut u8, wealth: u32, built: u32) -> u64 = "thiscall" 0x0069_CA80; }
game_fn! { #[allow(dead_code)] fn classify_state(state: u32) -> u64 = "C" 0x0069_5260; }
game_fn! { #[allow(dead_code)] fn append_special(this: *mut u8, occupant: *const u32) -> u64 = "thiscall" 0x0046_7B50; }
game_fn! { #[allow(dead_code)] fn find_special(out: *mut u32, first: u32, end: u32, occupant: *const u32) -> u64 = "C" 0x004D_C710; }
game_fn! { #[allow(dead_code)] fn erase_special(this: *mut u8, out: *mut u32, node: u32) -> u64 = "thiscall" 0x0041_3F70; }

layout! {
    /// Fields relative to the message-target interface, eight bytes into the simulator.
    pub struct MessageTarget {
        0x38 => cells_x / set_cells_x: i32,
        0x3C => cells_z / set_cells_z: i32,
        0x44 => special / set_special: *mut u8,
        0xD5 => ignore_plumbing / set_ignore_plumbing: u8,
        0xD6 => ignore_power / set_ignore_power: u8,
        0xD9 => coffee_bar / set_coffee_bar: u8,
    }
}

layout! {
    /// The shared getter's byte relative to each object's primary interface.
    pub struct DebugGrowthFlag {
        0xE0 => value / set_value: u8,
    }
}

#[derive(Clone, Copy)]
enum TableRegister {
    Eax,
    Edx,
}

struct Registers {
    eax: u32,
    edx: u32,
}

impl Registers {
    // SAFETY for these helpers: the enclosing entry supplies live game interfaces and each selected
    // address or slot has the stack signature given at the call site.
    unsafe fn call(&mut self, conv: Conv, address: usize, this: u32, args: &[u32], st0: bool) -> F80 {
        let mut c = RawCall::new(conv, address, this, args);
        c.eax = self.eax;
        c.edx = self.edx;
        c.take_st0 = u32::from(st0);
        unsafe { call_raw(&mut c) };
        self.eax = c.eax;
        self.edx = c.edx;
        c.st0
    }

    unsafe fn virtual_call(&mut self, object: *mut u8, slot: usize, args: &[u32], table: TableRegister) {
        unsafe { self.virtual_result(object, slot, args, table, false) };
    }

    unsafe fn virtual_result(
        &mut self,
        object: *mut u8,
        slot: usize,
        args: &[u32],
        table: TableRegister,
        st0: bool,
    ) -> F80 {
        let table_word = unsafe { read(object.cast::<u32>()) };
        match table {
            TableRegister::Eax => self.eax = table_word,
            TableRegister::Edx => self.edx = table_word,
        }
        unsafe { self.call(Conv::Thiscall, vslot(object, slot), object as u32, args, st0) }
    }

    unsafe fn game(&mut self, address: u32, this: *mut u8, args: &[u32]) {
        unsafe { self.call(Conv::Thiscall, target(address), this as u32, args, false) };
    }

    fn result(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from((self.eax & !0xFF) | 1)
    }
}

port! {
    /// Dispatches simulation, cheat, command, occupant, terrain and lot-state messages.
    /// AL is always one; the remaining EAX bytes and EDX retain the last operation's values.
    /// The occupant and state cases also overwrite the caller's message argument word.
    #[port(address = 0x0069_CCE0, owner = "building", name = "cSC4BuildingDevelopmentSimulator::DoMessage", naked)]
    pub unsafe extern "C" fn fc4p_0069cce0() {
        core::arch::naked_asm!(
            "sub esp, 20",
            "lea eax, [esp + 24]", "push dword ptr [esp + 24]", "push eax",
            "call {body}", "add esp, 20", "ret 4", body = sym do_message,
        );
    }
}

/// # Safety
/// `this` is the live message-target interface, `message` its message, and `argument` the writable
/// original stack argument. The entry reserves twenty bytes immediately below its return address,
/// keeping rectangle, position and iterator scratch at the original addresses. All selected callbacks
/// have the game's signatures.
unsafe extern "fastcall" fn do_message(this: *mut u8, entry_edx: u32, argument: *mut u32, message: *mut u8) -> u64 {
    use TableRegister::{Eax, Edx};
    // SAFETY: the entry's live objects, original argument word, and game callback contracts.
    unsafe {
        let sim = MessageTarget::from_ptr(this);
        let primary = this.sub(8);
        let mut r = Registers { eax: 0, edx: entry_edx };
        r.virtual_call(message, 0x10, &[], Eax);
        let kind = r.eax;
        match kind {
            0x230E_27AC => {
                r.virtual_call(message, 0x24, &[], Edx);
                match r.eax {
                    0x08DA_D89A => sim.set_ignore_power(1),
                    0x4918_65E1 => sim.set_ignore_plumbing(1),
                    0x6AB6_A2D7 => sim.set_coffee_bar(u8::from(sim.coffee_bar() == 0)),
                    _ => {}
                }
            }
            0x0B99_AE81 => {
                r.virtual_call(message, 0x28, &[], Edx);
                let params = r.eax as *mut u8;
                r.virtual_call(params, 0x14, &[0], Eax);
                r.virtual_call(r.eax as *mut u8, 0x60, &[], Edx);
                let building = r.eax;
                r.virtual_call(params, 0x14, &[1], Eax);
                let position = argument.wrapping_sub(5).cast::<f32>();
                write(
                    position,
                    r.virtual_result(r.eax as *mut u8, 0x90, &[], Edx, true).to_f32(),
                );
                r.virtual_call(params, 0x14, &[2], Eax);
                write(
                    position.add(1),
                    r.virtual_result(r.eax as *mut u8, 0x90, &[], Edx, true).to_f32(),
                );
                r.eax = read(primary.cast());
                r.edx = position as u32;
                write(position.add(2), -1.0);
                r.call(
                    Conv::Thiscall,
                    vslot(primary, 0x1C),
                    primary as u32,
                    &[building, position as u32, 2],
                    false,
                );
            }
            0x278E_6003 => {
                r.virtual_call(message, 0x24, &[], Eax);
                let packed_x = r.eax;
                r.virtual_call(message, 0x38, &[], Edx);
                let packed_z = r.eax;
                r.virtual_call(message, 0x60, &[], Eax);
                let left = ((packed_x >> 16) as i32).wrapping_sub(1);
                let top = ((packed_z >> 16) as i32).wrapping_sub(1);
                let right = (packed_x & 0xFFFF) as i32;
                let bottom = (packed_z & 0xFFFF) as i32;
                let right = if right >= sim.cells_x() {
                    sim.cells_x().wrapping_sub(1)
                } else {
                    right
                };
                let bottom = if bottom >= sim.cells_z() {
                    sim.cells_z().wrapping_sub(1)
                } else {
                    bottom
                };
                let rect = argument.wrapping_sub(5).cast::<[i32; 4]>();
                write(rect, [left.max(0), top.max(0), right, bottom]);
                let data4 = r.eax;
                r.edx = top as u32;
                r.game(0x0069_8610, primary, &[rect as u32, data4]);
            }
            0x6695_6810 | 0x6695_6816 => {
                if kind == 0x6695_6810 {
                    r.eax = 0;
                }
                r.game(0x0069_6130, primary, &[]);
            }
            0x6695_6814 => {
                r.eax = 0;
                r.virtual_call(message, 0x4C, &[], Edx);
                let day = r.eax;
                r.game(0x0069_9EA0, primary, &[5]);
                r.game(0x0069_B6D0, primary, &[day]);
            }
            0x6695_681A => {
                r.virtual_call(message, 0x4C, &[], Eax);
                let budget = r.eax;
                r.game(0x0069_9EA0, primary, &[budget]);
            }
            0x99EF_1142 | 0x99EF_1143 => {
                let inserted = kind == 0x99EF_1142;
                r.virtual_call(message, 0x28, &[], if inserted { Eax } else { Edx });
                let occupant = r.eax as *mut u8;
                write(argument, r.eax);
                r.virtual_call(occupant, 0x18, &[], if inserted { Edx } else { Eax });
                r.virtual_call(r.eax as *mut u8, 0x0C, &[0x0292_44DB], Edx);
                if r.eax as u8 != 0 {
                    return r.result();
                }
                r.virtual_call(occupant, 0x58, &[0x150A], Eax);
                if r.eax as u8 == 0 {
                    r.virtual_call(occupant, 0x58, &[0x150B], Edx);
                    if r.eax as u8 == 0 {
                        return r.result();
                    }
                }
                let list = this.add(0x44);
                if inserted {
                    r.eax = argument as u32;
                    r.game(0x0046_7B50, list, &[argument as u32]);
                    r.virtual_call(occupant, 4, &[], Edx);
                } else {
                    let sentinel = sim.special();
                    let first = read(sentinel.cast::<u32>());
                    let out = argument.wrapping_sub(6);
                    r.eax = argument.wrapping_sub(13) as u32;
                    r.edx = out as u32;
                    r.call(
                        Conv::Cdecl,
                        target(0x004D_C710),
                        first,
                        &[out as u32, first, sentinel as u32, argument as u32],
                        false,
                    );
                    let node = read(out) as *mut u8;
                    r.eax = node as u32;
                    if node == sim.special() {
                        return r.result();
                    }
                    let removed = read(node.add(8).cast::<*mut u8>());
                    r.eax = argument as u32;
                    r.game(0x0041_3F70, list, &[argument as u32, node as u32]);
                    r.virtual_call(removed, 8, &[], Edx);
                }
            }
            0xE8BD_A2E7 => {
                r.virtual_call(message, 0x80, &[], Eax);
                let lot = r.eax as *mut u8;
                if lot.is_null() {
                    return r.result();
                }
                r.virtual_call(lot, 0xB8, &[0], Edx);
                if r.eax == 0 {
                    return r.result();
                }
                r.virtual_call(message, 0x24, &[], Eax);
                let old = r.eax;
                r.virtual_call(message, 0x38, &[], Edx);
                let new = r.eax;
                r.call(Conv::Cdecl, target(0x0069_5260), message as u32, &[new], false);
                let built = r.eax as u8;
                write(argument.cast::<u8>(), built);
                r.call(Conv::Cdecl, target(0x0069_5260), message as u32, &[old], false);
                if built != r.eax as u8 {
                    r.virtual_call(message, 0x60, &[], Eax);
                    let wealth = r.eax;
                    r.game(0x0069_CA80, primary, &[lot as u32, wealth, read(argument)]);
                }
            }
            _ => {
                if kind > 0x278E_6003 && kind < 0x6695_6816 {
                    r.eax = kind.wrapping_sub(0x6695_6814);
                }
            }
        }
        r.result()
    }
}

port! {
    /// Reads the debug-growth byte into AL, preserving EAX's upper bytes and EDX.
    #[port(address = 0x0098_AD60, owner = "building", name = "GetDebugGrowthFlag", naked)]
    pub unsafe extern "C" fn fc4p_0098ad60() {
        core::arch::naked_asm!("mov al, byte ptr [ecx + 0xe0]", "ret");
    }
}

pub const PORTS: &[Port] = &[fc4p_0069cce0::PORT, fc4p_0098ad60::PORT];

#[cfg(test)]
mod tests;
