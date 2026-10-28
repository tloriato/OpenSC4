// SPDX-License-Identifier: GPL-3.0-or-later
//! Disposal through exports, plants, landfill transport and decomposition.

use super::*;

game_fn! { fn curve_output(curve: *mut u8, value: f32) -> St0 = "thiscall" 0x005F_4250; }
game_data! { static transport_curve: *mut u8 = 0x00B4_9BC0; }
game_data! { static decomposition_limit: i32 = 0x00B4_9B70; }

layout! {
    /// Disposal inputs and counters, relative to the primary pollution interface.
    struct DisposalView {
        0x3C => landfill / set_landfill: *mut u8,
        0x150 => exported / set_exported: i64,
        0x160 => incinerated / set_incinerated: i64,
        0x168 => energy / set_energy: i64,
        0x1B0 => export_capacity / set_export_capacity: i32,
        0x1B4 => decomposed / set_decomposed: u32,
        0x228 => efficiency / set_efficiency: f32,
        0x22C => supply / set_supply: i32,
    }
}

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = [0u8; 10];
            let input = value.to_bytes();
            // SAFETY: the memory operands are live; one x87 register is pushed and popped.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) &input, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(add_float, "fadd dword ptr [{operand}]");
stored_op!(subtract_float, "fsub dword ptr [{operand}]");
stored_op!(multiply_float, "fmul dword ptr [{operand}]");

fn unsigned(value: u32) -> F80 {
    let value80 = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        value80
    } else {
        add_float(value80, 4_294_967_296.0)
    }
}

fn positive(value: f32) -> bool {
    let comparison = F80::from_f32(value).compare(F80::ZERO);
    !comparison.less() && !comparison.equal() && !comparison.unordered()
}

fn bias(value: F80) -> u32 {
    add_float(value, 12_582_912.0)
        .to_f32()
        .to_bits()
        .wrapping_add(0xB4C0_0000)
}

fn floor_bias(value: f32) -> u32 {
    let result = bias(subtract_float(F80::from_f32(value), 0.5));
    let next = result.wrapping_add(1);
    if F80::from_f32(value).compare(F80::from_i32(next as i32)).equal() {
        next
    } else {
        result
    }
}

unsafe fn compute(this: *mut u8, mut amount: f32, incinerator: *mut u32, energy: *mut u32) -> F80 {
    // SAFETY: all fields, capacity words and interfaces are the original live game objects.
    unsafe {
        let sim = DisposalView::from_ptr(this);
        let mut taken = F80::ZERO;
        let mut saved_taken = 0.0;
        if !positive(amount) {
            return taken;
        }
        let export_room = sim.export_capacity().wrapping_sub(sim.exported() as i32);
        if export_room > 0 {
            let room = F80::from_i32(export_room);
            let offered = F80::from_f32(amount);
            let comparison = offered.compare(room);
            let exported = if comparison.less() || comparison.equal() {
                amount = 0.0;
                offered
            } else {
                amount = (offered - room).to_f32();
                room
            };
            sim.set_exported((F80::from_i64(sim.exported()) + exported).ftol());
            taken = exported;
            saved_taken = taken.to_f32();
        }
        if !positive(amount) {
            return taken;
        }
        for (remaining, offset) in [(energy, 0x168), (incinerator, 0x160)] {
            let capacity = read_unaligned(remaining);
            if capacity != 0 {
                let comparison = unsigned(capacity).compare(F80::from_f32(amount));
                let consumed = if !comparison.less() && !comparison.equal() && !comparison.unordered() {
                    let consumed = F80::from_f32(amount);
                    write_unaligned(remaining, capacity.wrapping_sub(floor_bias(amount)));
                    amount = 0.0;
                    consumed
                } else {
                    // Exhausting a plant clears its remaining capacity but consumes none of this offer.
                    write_unaligned(remaining, 0);
                    F80::ZERO
                };
                let counter = this.add(offset).cast::<i64>();
                let updated = (F80::from_i64(read_unaligned(counter)) + consumed).ftol();
                taken = taken + consumed;
                write_unaligned(counter, updated);
                saved_taken = taken.to_f32();
            }
            if !positive(amount) {
                return taken;
            }
        }
        let mut used = 0i32;
        let mut capacity = 0i32;
        vcall!(sim.landfill(), 0x28 => fn(*mut i32,*mut i32), &mut used,&mut capacity);
        let room = capacity.wrapping_sub(used);
        let efficiency = sim.efficiency();
        let tiles = vcall!(sim.landfill(), 0x2C => fn() -> i32);
        let input = F80::from_i32(tiles).to_f32();
        let limit = bias(multiply_float(curve_output(*transport_curve(), input), efficiency))
            .wrapping_sub(sim.supply() as u32) as i32;
        let room = if limit < room { limit } else { room };
        if room > 0 {
            let room_float = F80::from_i32(room).to_f32();
            let comparison = F80::from_f32(amount).compare(F80::from_f32(room_float));
            if comparison.less() || comparison.equal() {
                vcall!(this,0x74 => fn(u32),bias(F80::from_f32(amount)));
                taken = add_float(F80::from_f32(amount), saved_taken);
                amount = 0.0;
            } else {
                vcall!(this,0x74 => fn(i32),room);
                amount = subtract_float(F80::from_f32(amount), room_float).to_f32();
                taken = add_float(F80::from_f32(room_float), saved_taken);
            }
        } else {
            taken = F80::from_f32(saved_taken);
        }
        if positive(amount) {
            let current = sim.decomposed();
            let room = (*decomposition_limit()).wrapping_sub(current as i32);
            if room > 0 {
                let room = F80::from_i32(room);
                let comparison = F80::from_f32(amount).compare(room);
                let decomposed = if comparison.less() || comparison.equal() {
                    F80::from_f32(amount)
                } else {
                    room
                };
                let updated = (unsigned(current) + decomposed).ftol();
                taken = taken + decomposed;
                sim.set_decomposed(updated as u32);
            }
        }
        taken
    }
}

unsafe extern "fastcall" fn body(
    this: *mut u8,
    output: *mut F80,
    amount: f32,
    incinerator: *mut u32,
    energy: *mut u32,
) {
    // SAFETY: the entry wrapper supplies a writable extended-precision result and forwards all arguments.
    unsafe { output.write_unaligned(compute(this, amount, incinerator, energy)) };
}

port! {
    /// Returns the unrounded x87 amount disposed, and updates capacity words and the original counters.
    /// The entry wrapper forwards the thiscall arguments to the Rust body and loads its ten-byte result.
    #[port(address = 0x006E_3820, owner = "pollution", name = "cSC4PollutionSimulator::GetUnusedCapacity", naked)]
    pub unsafe extern "C" fn fc4p_006e3820() {
        core::arch::naked_asm!(
            "sub esp, 12",
            "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]",
            "push dword ptr [esp + 24]",
            "lea edx, [esp + 12]",
            "call {body}",
            "fld tbyte ptr [esp]",
            "add esp, 12",
            "ret 12",
            body = sym body,
        );
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3820::PORT];

#[cfg(test)]
mod tests;
