// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Monthly deal activation, failure periods, renewal and deferred removal.

use super::{pair, pointer, virtual_call, word, NeighborsSim};
use crate::abi::{call_raw, target, vslot, Conv, RawCall};
use crate::port::Port;
use crate::x87::F80;
use core::mem::MaybeUninit;

// Raw calls also supply the integer scratch registers retained by short callees.
game_fn! { #[allow(dead_code)] fn post_message(id: u32, a: u32, b: u32, c: u32, d: u32, e: u32) -> u64 = "C" 0x005F_98C0; }
game_fn! { #[allow(dead_code)] fn clear_removals(this: *mut u8) -> u64 = "thiscall" 0x0050_A2A0; }
game_fn! { #[allow(dead_code)] fn property_float(holder: *mut u8, id: u32, out: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { #[allow(dead_code)] fn allocate(size: u32) -> *mut u8 = "C" 0x0090_CF54; }
game_fn! { #[allow(dead_code)] fn free(node: *mut u8) = "C" 0x0090_CF63; }
// The report helper receives the deal in ECX and two caller-popped stack words.
game_fn! { #[allow(dead_code)] fn report(send: u32, status: u32) -> u64 = "C" 0x006C_F5C0; }
game_fn! { #[allow(dead_code)] fn append_removal(this: *mut u8, cursor: *mut *mut u8) = "thiscall" 0x0042_7640; }
game_fn! { #[allow(dead_code)] fn erase_deal(this: *mut u8, out: *mut u32, node: *mut u8) = "thiscall" 0x0042_16E0; }
game_fn! { #[allow(dead_code)] fn publish(this: *mut u8, deals: u32, stats: u32) -> u64 = "thiscall" 0x006D_0A40; }
game_fn! { #[allow(dead_code)] fn date_new(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_3177; }
game_fn! { #[allow(dead_code)] fn date_from_interface(this: *mut u8, date: *mut u8) -> *mut u8 = "thiscall" 0x0091_2DFA; }
game_fn! { #[allow(dead_code)] fn date_from_day(this: *mut u8, day: u32) -> *mut u8 = "thiscall" 0x0091_2E7B; }
game_fn! { #[allow(dead_code)] fn date_destroy(this: *mut u8) = "thiscall" 0x0091_28BF; }
game_fn! { #[allow(dead_code)] fn day_number(this: *mut u8) -> u32 = "thiscall" 0x0091_297E; }
game_fn! { #[allow(dead_code)] fn date_at_least(today: *mut u8, date: *mut u8) -> u32 = "C" 0x006C_E6A0; }
game_data! { static current_city: *mut u8 = 0x00B4_3CEC; }
game_data! { static region: *mut u8 = 0x00B4_3C94; }
game_data! { static clock: *mut u8 = 0x00B4_3D5C; }
game_data! { static landfill: *mut u8 = 0x00B4_3D28; }
game_data! { static twelve: f32 = 0x00AA_209C; }
game_data! { static epsilon: f32 = 0x00AB_1F7C; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }

layout! {
    /// Both lists use a circular sentinel and two links followed by a payload word.
    struct Node {
        0x00 => next / set_next: *mut u8,
        0x04 => previous / set_previous: *mut u8,
        0x08 => value / set_value: *mut u8,
    }
}

#[derive(Clone, Copy)]
struct Registers {
    eax: u32,
    edx: u32,
}

impl Registers {
    unsafe fn call(&mut self, address: u32, conv: Conv, this: *mut u8, args: &[u32]) {
        let mut c = RawCall::new(conv, target(address), this as u32, args);
        c.eax = self.eax;
        c.edx = self.edx;
        unsafe { call_raw(&mut c) };
        self.eax = c.eax;
        self.edx = c.edx;
    }

    /// Loads the virtual table into EAX or EDX before calling the slot.
    unsafe fn virtual_call(&mut self, object: *mut u8, slot: usize, args: &[u32], table_in_eax: bool) {
        unsafe {
            if table_in_eax {
                self.eax = word(object, 0);
            } else {
                self.edx = word(object, 0);
            }
            let result = virtual_call(object, slot, args, self.eax, self.edx);
            self.eax = result as u32;
            self.edx = (result >> 32) as u32;
        }
    }

    unsafe fn float_call(&mut self, object: *mut u8, slot: usize, args: &[u32], table_in_eax: bool) -> F80 {
        unsafe {
            if table_in_eax {
                self.eax = word(object, 0);
            } else {
                self.edx = word(object, 0);
            }
            let mut c = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
            c.eax = self.eax;
            c.edx = self.edx;
            c.take_st0 = 1;
            call_raw(&mut c);
            self.eax = c.eax;
            self.edx = c.edx;
            c.st0
        }
    }

    unsafe fn report(&mut self, deal: *mut u8, status: u32) {
        unsafe { self.call(0x006C_F5C0, Conv::Cdecl, deal, &[1, status]) };
    }

    unsafe fn now(&mut self) -> u32 {
        unsafe {
            self.virtual_call(clock().read(), 0x1C, &[], true);
        }
        self.eax
    }

    fn ftol(&mut self, value: F80) {
        let result = value.ftol() as u64;
        self.eax = result as u32;
        self.edx = (result >> 32) as u32;
    }
}

// Memory operands stay memory operands so signaling NaNs participate in the operation itself.
macro_rules! stored_op {
    ($name:ident, $ty:ty, $instruction:literal) => {
        fn $name(value: F80, operand: $ty) -> F80 {
            let mut output = F80::ZERO;
            // SAFETY: one x87 value is loaded and popped; both operands are live local values.
            unsafe {
                core::arch::asm!(
                    "fld tbyte ptr [{value}]", $instruction, "fstp tbyte ptr [{output}]",
                    value = in(reg) &value, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _,
                );
            }
            output
        }
    };
}
stored_op!(add_float, f32, "fadd dword ptr [{operand}]");
stored_op!(multiply_float, f32, "fmul dword ptr [{operand}]");
stored_op!(divide_from_float, f32, "fdivr dword ptr [{operand}]");
stored_op!(subtract_from_float, f32, "fsubr dword ptr [{operand}]");
stored_op!(add_double, f64, "fadd qword ptr [{operand}]");
stored_op!(subtract_from_double, f64, "fsubr qword ptr [{operand}]");

port! {
    /// Simulates deals in list order, queues removals and erases them from the back of the queue.
    /// Nested calls post a retry. Silent entries recheck the same cursor until their flag changes.
    /// The entry retains EAX and EDX for the early exits and short callees.
    #[port(address = 0x006D_0E50, owner = "neighbors", name = "cSC4NeighborsSimulator::simulate_deals", naked)]
    pub unsafe extern "C" fn fc4p_006d0e50() {
        core::arch::naked_asm!("push eax", "call {}", "ret", sym simulate);
    }
}

unsafe extern "fastcall" fn simulate(this: *mut u8, entry_edx: u32, entry_eax: u32) -> u64 {
    // SAFETY: the game supplies the live simulator, lists, services, properties and deal interfaces.
    unsafe {
        let sim = NeighborsSim::from_ptr(this);
        let mut r = Registers {
            eax: (entry_eax & !0xFF) | u32::from(sim.initialized()),
            edx: entry_edx,
        };
        if sim.initialized() == 0 {
            return pair(r.eax, r.edx);
        }
        r.eax = (r.eax & !0xFF) | u32::from(sim.busy());
        if sim.busy() != 0 {
            r.call(0x005F_98C0, Conv::Cdecl, this, &[0xEA41_8D8F, 8, 0, 0, 0, 0]);
            return pair(r.eax, r.edx);
        }
        sim.set_busy(1);
        r.virtual_call(this, 0x88, &[current_city().read() as u32], true);
        r.eax = (r.eax & !0xFF) | u32::from(sim.ui_disabled());
        if sim.ui_disabled() == 0 {
            run_deals(sim, &mut r);
        }
        let deals = u32::from(sim.deals_dirty());
        r.edx = deals;
        r.call(
            0x006D_0A40,
            Conv::Thiscall,
            this,
            &[deals, u32::from(sim.stats_dirty())],
        );
        sim.set_busy(0);
        pair(r.eax, r.edx)
    }
}

unsafe fn run_deals(sim: NeighborsSim, r: &mut Registers) {
    unsafe {
        let this = sim.as_ptr();
        let pending = this.add(0x24);
        r.call(0x0050_A2A0, Conv::Thiscall, pending, &[]);
        r.virtual_call(this, 0x94, &[], false);
        let resource = r.eax as *mut u8;
        r.virtual_call(resource, 0x18, &[], false);
        let holder = r.eax as *mut u8;
        let mut term = 0.0f32;
        r.call(
            0x005F_D4F0,
            Conv::Cdecl,
            holder,
            &[holder as u32, 0xEA46_934F, &mut term as *mut f32 as u32],
        );
        let months = multiply_float(F80::from_f32(term), twelve().read()).to_f32();
        let (mut used, mut capacity) = (0u32, 0u32);
        r.eax = &mut used as *mut u32 as u32;
        r.virtual_call(
            landfill().read(),
            0x28,
            &[&mut used as *mut u32 as u32, &mut capacity as *mut u32 as u32],
            false,
        );
        let free = capacity.wrapping_sub(used);
        r.virtual_call(region().read(), 0x90, &[], false);
        let regional = r.eax as *mut u8;
        let mut exported = r.float_call(regional, 0xD8, &[3], true).to_f64();
        let produced = r.float_call(regional, 0xE8, &[3], false).to_f64();
        let subtotal = add_double(r.float_call(regional, 0xE0, &[3], true), produced);
        let factor = subtract_from_float(
            divide_from_float(add_float(F80::from_f32(months), epsilon().read()), one().read()),
            one().read(),
        );
        let mut free80 = F80::from_i32(free as i32);
        if (free as i32) < 0 {
            free80 = add_float(free80, unsigned_bias().read());
        }
        let limit = (subtotal + factor * free80).to_f64();
        r.eax = sim.deals() as u32;
        let mut cursor = pointer(sim.deals(), 0);
        while cursor != sim.deals() {
            let deal = pointer(cursor, 8);
            r.virtual_call(deal, 0x9C, &[0x80], false);
            if r.eax as u8 == 0 {
                r.virtual_call(deal, 0x84, &[], true);
                r.virtual_call(deal, 0x20, &[], false);
                if r.eax == 0x4773_FFE5 {
                    let comparison = F80::from_f64(exported).compare(F80::from_f64(limit));
                    r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
                    if comparison.status_word() & 0x4100 == 0 {
                        r.virtual_call(deal, 0x28, &[], true);
                        exported = subtract_from_double(F80::from_i32(r.eax as i32), exported).to_f64();
                        r.virtual_call(deal, 0x94, &[0x20, 1], false);
                    }
                }
                r.virtual_call(deal, 0x9C, &[0x3C], true);
                if r.eax as u8 != 0 {
                    failure(sim, deal, &mut cursor, r);
                } else {
                    r.virtual_call(deal, 0x9C, &[1], true);
                    if r.eax as u8 != 0 {
                        r.virtual_call(deal, 0x8C, &[], false);
                        if r.eax as u8 != 0 {
                            set_time(deal, 0x74, r);
                            r.report(deal, 1);
                            sim.set_stats_dirty(1);
                        }
                    } else {
                        r.virtual_call(deal, 0x9C, &[2], false);
                        if r.eax as u8 != 0 {
                            active(deal, regional, holder, r);
                        } else {
                            r.virtual_call(deal, 0x88, &[], false);
                        }
                    }
                }
                cursor = pointer(cursor, 0);
            }
            // Silent deals deliberately keep the current cursor.
            r.edx = cursor as u32;
            r.eax = sim.deals() as u32;
        }
        sim.set_deals_dirty(u8::from(
            sim.deals_dirty() != 0 || pointer(sim.pending_removals(), 0) != sim.pending_removals(),
        ));
        while pointer(sim.pending_removals(), 0) != sim.pending_removals() {
            let node = pointer(sim.pending_removals(), 4);
            let selected = pointer(node, 8);
            let mut out = 0u32;
            r.eax = &mut out as *mut u32 as u32;
            r.edx = r.eax;
            r.call(
                0x0042_16E0,
                Conv::Thiscall,
                this.add(0x20),
                &[&mut out as *mut u32 as u32, selected as u32],
            );
            let node = Node::from_ptr(pointer(sim.pending_removals(), 4));
            let next = node.next();
            let previous = node.previous();
            Node::from_ptr(previous).set_next(next);
            Node::from_ptr(next).set_previous(previous);
            r.eax = node.as_ptr() as u32;
            r.edx = previous as u32;
            r.call(0x0090_CF63, Conv::Cdecl, next, &[node.as_ptr() as u32]);
        }
        r.eax = current_city().read() as u32;
        r.virtual_call(this, 0x88, &[r.eax], false);
    }
}

unsafe fn failure(sim: NeighborsSim, deal: *mut u8, cursor: &mut *mut u8, r: &mut Registers) {
    unsafe {
        r.virtual_call(deal, 0x9C, &[4], false);
        let status;
        if r.eax as u8 != 0 {
            let header = sim.pending_removals();
            r.call(0x0090_CF54, Conv::Cdecl, deal, &[12]);
            let node = Node::from_ptr(r.eax as *mut u8);
            if (r.eax as *mut u8).wrapping_add(8) as u32 != 0 {
                node.set_value(*cursor);
            }
            let previous = Node::from_ptr(header).previous();
            node.set_previous(previous);
            node.set_next(header);
            Node::from_ptr(previous).set_next(node.as_ptr());
            Node::from_ptr(header).set_previous(node.as_ptr());
            r.eax = node.as_ptr() as u32;
            r.edx = *cursor as u32;
            status = 2;
        } else {
            r.virtual_call(deal, 0x9C, &[0x200], false);
            if r.eax as u8 == 0 {
                r.virtual_call(deal, 0x94, &[0x100, 1], true);
                r.virtual_call(deal, 0x94, &[0x38, 0], false);
                set_time(deal, 0x7C, r);
                return;
            }
            r.virtual_call(deal, 0x9C, &[8], false);
            if r.eax as u8 != 0 {
                status = 3;
                r.eax = cursor as *mut *mut u8 as u32;
            } else {
                r.virtual_call(deal, 0x9C, &[0x10], true);
                if r.eax as u8 != 0 {
                    status = 4;
                } else {
                    r.virtual_call(deal, 0x9C, &[0x20], true);
                    if r.eax as u8 == 0 {
                        return;
                    }
                    status = 5;
                }
            }
            r.call(
                0x0042_7640,
                Conv::Thiscall,
                sim.as_ptr().add(0x24),
                &[cursor as *mut *mut u8 as u32],
            );
        }
        r.report(deal, status);
        r.virtual_call(deal, 0x2C, &[0], status == 2);
    }
}

unsafe fn set_time(deal: *mut u8, slot: usize, r: &mut Registers) {
    unsafe {
        let clock = clock().read();
        r.eax = word(clock, 0);
        let table = word(deal, 0);
        let result = virtual_call(clock, 0x1C, &[], r.eax, r.edx);
        r.eax = result as u32;
        r.edx = (result >> 32) as u32;
        let args = [r.eax];
        let mut c = RawCall::new(
            Conv::Thiscall,
            word(table as *mut u8, slot) as usize,
            deal as u32,
            &args,
        );
        c.eax = r.eax;
        c.edx = r.edx;
        call_raw(&mut c);
        r.eax = c.eax;
        r.edx = c.edx;
    }
}

unsafe fn active(deal: *mut u8, regional: *mut u8, holder: *mut u8, r: &mut Registers) {
    unsafe {
        let mut expiration = MaybeUninit::<[u32; 4]>::uninit();
        let expiration = expiration.as_mut_ptr().cast::<u8>();
        r.call(0x0091_3177, Conv::Thiscall, expiration, &[]);
        let time = r.now();
        let mut today = MaybeUninit::<[u32; 4]>::uninit();
        let today = today.as_mut_ptr().cast::<u8>();
        r.call(0x0091_2DFA, Conv::Thiscall, today, &[time]);
        r.eax = today.add(8) as u32;
        r.virtual_call(deal, 0x74, &[today.add(8) as u32], false);
        r.eax = expiration.add(8) as u32;
        r.virtual_call(deal, 0x68, &[expiration.add(8) as u32], false);
        r.virtual_call(deal, 0x9C, &[0x100], false);
        if r.eax as u8 != 0 {
            period(deal, holder, today, false, r);
        } else {
            r.virtual_call(deal, 0x9C, &[0x200], false);
            if r.eax as u8 != 0 {
                period(deal, holder, today, true, r);
            } else {
                r.virtual_call(deal, 0x98, &[0x40], true);
                if r.eax as u8 != 0 {
                    renew(deal, regional, r);
                } else {
                    r.edx = today as u32;
                    r.call(0x006C_E6A0, Conv::Cdecl, today, &[today as u32, expiration as u32]);
                    if r.eax as u8 != 0 {
                        r.virtual_call(deal, 0x94, &[0x40, 1], true);
                        r.report(deal, 6);
                    }
                }
            }
        }
        r.call(0x0091_28BF, Conv::Thiscall, today, &[]);
        r.call(0x0091_28BF, Conv::Thiscall, expiration, &[]);
    }
}

unsafe fn period(deal: *mut u8, holder: *mut u8, today: *mut u8, probation: bool, r: &mut Registers) {
    unsafe {
        let mut probation_days = 0.0f32;
        let mut grace_days = 0.0f32;
        if probation {
            r.eax = &mut probation_days as *mut f32 as u32;
            r.call(
                0x005F_D4F0,
                Conv::Cdecl,
                holder,
                &[holder as u32, 0x8B58_C59E, &mut probation_days as *mut f32 as u32],
            );
            r.eax = holder as u32;
        } else {
            r.eax = &mut grace_days as *mut f32 as u32;
        }
        r.call(
            0x005F_D4F0,
            Conv::Cdecl,
            holder,
            &[holder as u32, 0x6B58_8FAD, &mut grace_days as *mut f32 as u32],
        );
        let mut date = MaybeUninit::<[u32; 4]>::uninit();
        let date = date.as_mut_ptr().cast::<u8>();
        r.call(0x0091_3177, Conv::Thiscall, date, &[]);
        r.eax = date.add(8) as u32;
        r.virtual_call(deal, 0x78, &[date.add(8) as u32], false);
        let days = if probation {
            add_float(F80::from_f32(grace_days), probation_days)
        } else {
            F80::from_f32(grace_days)
        };
        r.ftol(days);
        r.virtual_call(date.add(8), 0x9C, &[r.eax], false);
        r.eax = date as u32;
        r.call(0x006C_E6A0, Conv::Cdecl, today, &[today as u32, date as u32]);
        if r.eax as u8 != 0 {
            r.virtual_call(deal, 0x94, &[if probation { 0x200 } else { 0x100 }, 0], false);
            if !probation {
                r.virtual_call(deal, 0x94, &[0x200, 1], true);
            }
        }
        r.call(0x0091_28BF, Conv::Thiscall, date, &[]);
    }
}

unsafe fn renew(deal: *mut u8, regional: *mut u8, r: &mut Registers) {
    unsafe {
        r.virtual_call(deal, 0x94, &[0x40, 0], false);
        r.virtual_call(deal, 0x20, &[], true);
        let mut payer = regional;
        r.eax = r.eax.wrapping_sub(0x4773_FFE1);
        if r.eax != 0 {
            r.eax = r.eax.wrapping_sub(2);
            if r.eax != 0 {
                r.eax = r.eax.wrapping_sub(2);
            }
        }
        if r.eax == 0 {
            r.virtual_call(deal, 0x1C, &[], false);
            payer = r.eax as *mut u8;
        }
        r.virtual_call(deal, 0x5C, &[], true);
        r.virtual_call(deal, 0x58, &[], false);
        let cost = F80::from_i64(pair(r.eax, r.edx) as i64).to_f64();
        let budget = r.float_call(payer, 0xC8, &[], true);
        let comparison = budget.compare(F80::from_f64(cost));
        r.eax = (r.eax & 0xFFFF_0000) | u32::from(comparison.status_word());
        if comparison.status_word() & 0x100 != 0 {
            r.virtual_call(deal, 0x94, &[0x10, 1], true);
            return;
        }
        let mut start = MaybeUninit::<[u32; 4]>::uninit();
        let start = start.as_mut_ptr().cast::<u8>();
        let mut end = MaybeUninit::<[u32; 4]>::uninit();
        let end = end.as_mut_ptr().cast::<u8>();
        let mut today = MaybeUninit::<[u32; 4]>::uninit();
        let today = today.as_mut_ptr().cast::<u8>();
        let mut renewed = MaybeUninit::<[u32; 4]>::uninit();
        let renewed = renewed.as_mut_ptr().cast::<u8>();
        r.call(0x0091_3177, Conv::Thiscall, start, &[]);
        r.call(0x0091_3177, Conv::Thiscall, end, &[]);
        r.virtual_call(clock().read(), 0x1C, &[], false);
        r.call(0x0091_2DFA, Conv::Thiscall, today, &[r.eax]);
        r.virtual_call(deal, 0x74, &[today.add(8) as u32], true);
        r.eax = start.add(8) as u32;
        r.virtual_call(deal, 0x60, &[r.eax], false);
        r.eax = end.add(8) as u32;
        r.virtual_call(deal, 0x68, &[r.eax], false);
        r.call(0x0091_297E, Conv::Thiscall, start.add(8), &[]);
        let first = r.eax;
        r.call(0x0091_297E, Conv::Thiscall, end.add(8), &[]);
        let span = r.eax.wrapping_sub(first);
        r.eax = today.add(8) as u32;
        r.virtual_call(deal, 0x64, &[r.eax], false);
        r.call(0x0091_297E, Conv::Thiscall, today.add(8), &[]);
        r.eax = r.eax.wrapping_add(span);
        r.call(0x0091_2E7B, Conv::Thiscall, renewed, &[r.eax]);
        r.eax = if r.eax == 0 { 0 } else { r.eax.wrapping_add(8) };
        r.virtual_call(deal, 0x6C, &[r.eax], false);
        r.call(0x0091_28BF, Conv::Thiscall, renewed, &[]);
        r.report(deal, 7);
        r.call(0x0091_28BF, Conv::Thiscall, today, &[]);
        r.call(0x0091_28BF, Conv::Thiscall, end, &[]);
        r.call(0x0091_28BF, Conv::Thiscall, start, &[]);
    }
}

pub const PORTS: &[Port] = &[fc4p_006d0e50::PORT];

#[cfg(test)]
mod tests;
