// SPDX-License-Identifier: GPL-3.0-or-later
//! Monthly facility aging and lot condition changes.

use super::*;

game_fn! { fn property_i32(holder: *mut u8, id: u32, output: *mut i32) = "C" 0x005F_D450; }
game_fn! { fn property_float(holder: *mut u8, id: u32, output: *mut f32) = "C" 0x005F_D4F0; }
game_fn! { fn set_float(holder: *mut u8, id: u32, value: f32) = "C" 0x005F_E4F0; }
game_fn! { fn curve_output(curve: *mut u8, value: f32) -> St0 = "thiscall" 0x005F_4250; }
game_data! { static default_ratio: f32 = 0x00B4_9BC4; }
game_data! { static city: *mut u8 = 0x00B4_3CEC; }
game_data! { static lots: *mut u8 = 0x00B4_3D08; }

layout! {
    /// Aging inputs in the primary pollution simulator.
    struct AgingView {
        0xD8 => department / set_department: *mut u8,
        0xFC => recycling_count / set_recycling_count: u32,
        0x158 => recycled / set_recycled: f64,
    }
}

fn unsigned(value: u32) -> F80 {
    let value80 = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        value80
    } else {
        value80 + F80::from_f32(4_294_967_296.0)
    }
}

// A memory operand keeps signaling-NaN handling and payload selection with the arithmetic operation.
macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = [0u8; 10];
            let input = value.to_bytes();
            // SAFETY: both operands are readable and one x87 register is pushed and popped.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) &input, operand = in(reg) &operand, output = in(reg) &mut output,
                    out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(multiply_float, "fmul dword ptr [{operand}]");
stored_op!(add_float, "fadd dword ptr [{operand}]");

fn below_or_unordered(value: f32, limit: f32) -> bool {
    F80::from_f32(value).compare(F80::from_f32(limit)).status_word() & 0x100 != 0
}

port! {
    /// Ages every occupant from its usage ratio, department funding, decay rate and two response curves.
    /// Missing properties retain the initialized defaults. Thresholds default to 99; a reached threshold
    /// finds the lot at the occupant's position and applies condition four before considering condition two.
    #[port(address = 0x006E_3D00, owner = "pollution", name = "cSC4PollutionSimulator::AgeOccupantList")]
    pub unsafe extern "thiscall" fn fc4p_006e3d00(this: *mut u8, list: *mut *mut u8, usage_property: u32, capacity_property: u32) {
        // SAFETY: live circular list, original property interfaces, curves, budget, city and lot services.
        unsafe {
            let sim = AgingView::from_ptr(this);
            let end = read_unaligned(list);
            let mut node = ListNode::from_ptr(end).next();
            while node != end {
                let occupant = ListNode::from_ptr(node).occupant();
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let mut used = 0i32;
                let mut capacity = 0u32;
                let mut ratio = *default_ratio();
                let mut condition = 0.0;
                let mut decay = 0.0;
                let mut threshold_four = 99.0;
                let mut threshold_two = 99.0;
                let funding = vcall!(sim.department(), 0x28 => fn() -> *const f32);
                if usage_property != 0 {
                    property_i32(holder, usage_property, &mut used);
                    vcall!(this, 0xD4 => fn(u32, *mut u8, *mut u32), capacity_property, occupant, &mut capacity);
                    if capacity != 0 && used > 0 {
                        ratio = (F80::from_i32(used) / unsigned(capacity)).to_f32();
                    }
                } else if sim.recycling_count() != 0 {
                    ratio = (F80::from_f64(sim.recycled()) / unsigned(sim.recycling_count())).to_f32();
                }
                property_float(holder, 0xC910_B4CF, &mut condition);
                property_float(holder, 0xEA1C_F220, &mut decay);
                property_float(holder, 0xEA1C_F221, &mut threshold_two);
                property_float(holder, 0xEA1C_F222, &mut threshold_four);
                let funding_value = read_unaligned(funding);
                let usage_factor = curve_output(this.add(0xEC), ratio).to_f32();
                let age = multiply_float(curve_output(this.add(0xDC), funding_value), usage_factor);
                condition = add_float(multiply_float(age, decay), condition).to_f32();
                set_float(holder, 0xC910_B4CF, condition);
                if !below_or_unordered(condition, threshold_two) || !below_or_unordered(condition, threshold_four) {
                    let mut position = [0.0f32; 3];
                    vcall!(occupant, 0x20 => fn(*mut f32), position.as_mut_ptr());
                    let mut x = 0i32;
                    let mut z = 0i32;
                    vcall!(*city(), 0x17C => fn(f32, f32, *mut i32, *mut i32), position[0], position[2], &mut x, &mut z);
                    let lot = vcall!(*lots(), 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 0);
                    if !lot.is_null() {
                        if !below_or_unordered(condition, threshold_four) {
                            vcall!(lot, 0x90 => fn(u32), 4);
                        } else if !below_or_unordered(condition, threshold_two) {
                            vcall!(lot, 0x90 => fn(u32), 2);
                        }
                    }
                }
                node = ListNode::from_ptr(node).next();
            }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e3d00::PORT];

#[cfg(test)]
mod tests;
