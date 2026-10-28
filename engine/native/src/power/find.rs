// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Independent searches for the lowest locally funded and most worn plants.

use super::*;
use crate::abi::{call_raw, vslot, Conv, RawCall};
use core::mem::MaybeUninit;
use core::ptr::read_unaligned;

game_fn! { fn get_float(holder: *mut u8, id: u32, out: *mut f32) -> u64 = "C" 0x005F_D4F0; }
stored_op!(divide_float, "fdiv dword ptr [{operand}]");

/// A stored comparison operand is not loaded separately, preserving its signaling-NaN behavior.
fn compare_float(value: F80, operand: f32) -> u16 {
    let status: u16;
    // SAFETY: both operands cover their values and the push/pop balance.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{value}]", "fcomp dword ptr [{operand}]", "fnstsw ax",
            value = in(reg) &value, operand = in(reg) &operand, out("ax") status, out("st(0)") _);
    }
    status
}
fn less(status: u16) -> bool {
    status & 0x4500 == 0x0100
}

#[derive(Clone, Copy)]
struct Registers {
    eax: u32,
    edx: u32,
}
impl Registers {
    unsafe fn invoke(&mut self, object: *mut u8, slot: usize, args: &[u32]) -> u32 {
        // SAFETY: the caller supplies a live interface and the slot's exact words.
        unsafe {
            let mut call = RawCall::new(Conv::Thiscall, vslot(object, slot), object as u32, args);
            call.eax = self.eax;
            call.edx = self.edx;
            call_raw(&mut call);
            self.eax = call.eax;
            self.edx = call.edx;
            call.eax
        }
    }
    fn value(self) -> u64 {
        (u64::from(self.edx) << 32) | u64::from(self.eax)
    }
}
unsafe fn table(object: *mut u8) -> u32 {
    // SAFETY: the caller's interface begins with its vtable word.
    unsafe { read_unaligned(object.cast()) }
}
unsafe fn occupant(node: *mut u8) -> *mut u8 {
    // SAFETY: the caller supplies a plant list node.
    unsafe { read_unaligned(node.add(12).cast()) }
}

port! {
    /// The entry forwards its original stack address so that the empty-result path can retain the
    /// upper EAX bytes of its temporary divisor address. The Rust body performs both searches and
    /// preserves the caller-visible EDX left by property and reference-count methods.
    #[port(address = 0x006E_BA30, owner = "power", name = "cSC4PowerSimulator::FindPowerPlants", naked)]
    pub unsafe extern "C" fn fc4p_006eba30() {
        core::arch::naked_asm!("mov eax, esp", "push eax", "call {body}", "ret 16", body = sym find_plants);
    }
}

unsafe extern "fastcall" fn find_plants(this: *mut u8, entry_edx: u32, stack: *mut u32) -> u64 {
    // SAFETY: the entry supplies the four original output arguments, live simulator and plant list.
    unsafe {
        let sim = PowerSim::from_ptr(this);
        let header = sim.plant_header();
        let mut node = read_unaligned(header.cast::<*mut u8>());
        let mut r = Registers {
            eax: header as u32,
            edx: entry_edx,
        };
        if node == header {
            return r.value();
        }
        let mut lowest: *mut u8 = core::ptr::null_mut();
        let mut worn: *mut u8 = core::ptr::null_mut();
        let (mut funding, mut ratio) = (2.0f32, 0.0f32);
        while node != header {
            let plant = occupant(node);
            r.eax = table(plant);
            let holder = r.invoke(plant, 0x18, &[]) as *mut u8;
            let mut record = MaybeUninit::<[u32; 8]>::uninit();
            let record_ptr = record.as_mut_ptr().cast::<u32>();
            record_ptr.add(7).write(0);
            r.eax = record_ptr as u32;
            r.edx = table(sim.budget());
            if r.invoke(sim.budget(), 0xCC, &[holder as u32, 0xCA58_E540, record_ptr as u32]) as u8 != 0 {
                r.edx = table(sim.department());
                if r.invoke(sim.department(), 0x70, &[holder as u32, 0xCA58_E540]) as u8 != 0 {
                    let status = compare_float(F80::from_f32(f32::from_bits(record_ptr.add(7).read())), funding);
                    if less(status) {
                        let new = occupant(node);
                        let old = lowest;
                        r.eax = old as u32;
                        if new != old {
                            if !new.is_null() {
                                r.eax = table(new);
                                r.invoke(new, 4, &[]);
                            }
                            lowest = new;
                            if !old.is_null() {
                                r.edx = table(old);
                                r.invoke(old, 8, &[]);
                            }
                        }
                        funding = f32::from_bits(record_ptr.add(7).read());
                    }
                }
            }
            let (mut wear, mut t1, mut t2) = (0.0f32, 99.0f32, 99.0f32);
            get_float(holder, 0xC910_B4CF, &mut wear);
            get_float(holder, 0xEA1C_F221, &mut t1);
            let result = get_float(holder, 0xEA1C_F222, &mut t2);
            r.edx = (result >> 32) as u32;
            let second_less = less(compare_float(F80::from_f32(t2), t1));
            let divisor = if second_less { t1 } else { t2 };
            let current = divide_float(F80::from_f32(wear), divisor).to_f32();
            let status = compare_float(F80::from_f32(ratio), current);
            let offset = if second_less { 0x3C } else { 0x38 };
            r.eax = ((stack as u32).wrapping_sub(offset) & 0xFFFF_0000) | u32::from(status);
            if less(status) {
                let new = occupant(node);
                if new != worn {
                    if !new.is_null() {
                        r.edx = table(new);
                        r.invoke(new, 4, &[]);
                    }
                    let old = worn;
                    worn = new;
                    if !old.is_null() {
                        r.eax = table(old);
                        r.invoke(old, 8, &[]);
                    }
                }
                ratio = current;
            }
            node = read_unaligned(node.cast());
        }
        let out_worn = stack.add(3).read() as *mut *mut u8;
        if !worn.is_null() && !out_worn.is_null() {
            out_worn.write_unaligned(worn);
            r.eax = out_worn as u32;
            r.edx = table(worn);
            r.invoke(worn, 4, &[]);
            let percentage = multiply_float(F80::from_f32(ratio), 100.0).ftol() as u64;
            r.eax = percentage as u32;
            r.edx = (percentage >> 32) as u32;
            (stack.add(4).read() as *mut u32).write_unaligned(r.eax);
        }
        let out_lowest = stack.add(1).read() as *mut *mut u8;
        if !lowest.is_null() && !out_lowest.is_null() {
            out_lowest.write_unaligned(lowest);
            r.eax = out_lowest as u32;
            r.edx = table(lowest);
            r.invoke(lowest, 4, &[]);
            r.eax = stack.add(2).read();
            (r.eax as *mut f32).write_unaligned(funding);
        }
        if !worn.is_null() {
            r.edx = table(worn);
            r.invoke(worn, 8, &[]);
        }
        if !lowest.is_null() {
            r.eax = table(lowest);
            r.invoke(lowest, 8, &[]);
        }
        r.value()
    }
}

pub const PORTS: &[Port] = &[fc4p_006eba30::PORT];

#[cfg(test)]
mod tests;
