// SPDX-License-Identifier: GPL-3.0-or-later
//! Facility capacity, condition-adjusted maintenance, and budget updates.

use super::*;

game_fn! { fn property_u32(holder: *mut u8, id: u32, output: *mut u32) -> u64 = "C" 0x005F_D480; }
game_fn! { fn property_i64(holder: *mut u8, id: u32, output: *mut i64) -> u64 = "C" 0x005F_D4B0; }
game_fn! { fn property_float(holder: *mut u8, id: u32, output: *mut f32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn curve_new(curve: *mut u32) -> u64 = "thiscall" 0x007D_0410; }
game_fn! { fn curve_set(curve: *mut u32, property: *mut u8) -> u64 = "thiscall" 0x005F_4620; }
game_fn! { #[allow(dead_code)] fn curve_output(curve: *mut u32, x: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { #[allow(dead_code)] fn free_curve(data: u32) -> u64 = "C" 0x0090_CF63; }
game_data! { static budget: *mut u8 = 0x00B4_3D64; }

layout! {
    /// Capacity scaling in the primary simulator.
    struct CapacityView {
        0x34 => occupants / set_occupants: *mut u8,
        0x228 => efficiency / set_efficiency: f32,
    }
}

macro_rules! stored_op {
    ($name:ident, $instruction:literal) => {
        fn $name(value: F80, operand: f32) -> F80 {
            let mut output = [0u8; 10];
            let input = value.to_bytes();
            // SAFETY: the pointers cover their operands; one x87 register is pushed and popped.
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

fn unsigned(value: u32) -> F80 {
    let signed = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        signed
    } else {
        add_float(signed, 4_294_967_296.0)
    }
}

unsafe fn evaluate(curve: *mut u32, x: f32, eax: u32, edx: u32) -> RawCall {
    let args = [x.to_bits()];
    let mut call = RawCall::new(Conv::Thiscall, crate::abi::target(0x005F_4250), curve as u32, &args);
    call.eax = eax;
    call.edx = edx;
    call.take_st0 = 1;
    // SAFETY: the original live curve and its floating-return method.
    unsafe { call_raw(&mut call) };
    call
}

unsafe fn dispose(data: u32, edx: u32) -> u64 {
    let args = [data];
    let mut call = RawCall::new(Conv::Cdecl, crate::abi::target(0x0090_CF63), 0, &args);
    call.eax = data;
    call.edx = edx;
    // SAFETY: the storage returned by the game's curve loader.
    unsafe { call_raw(&mut call) };
    registers(call.eax, call.edx)
}

port! {
    /// Reads nominal capacity, clears it for an inactive occupant, scales by efficiency with truncation,
    /// then optionally rounds its condition-curve product with the float bias formula. Missing nominal
    /// capacity writes zero and fails; a missing condition curve succeeds. AL alone is replaced.
    #[port(address = 0x006E_4300, owner = "pollution", name = "cSC4PollutionSimulator::GetCurrentCapacity")]
    pub unsafe extern "thiscall" fn fc4p_006e4300(this: *mut u8, id: u32, occupant: *mut u8, output: *mut u32) -> u64 {
        // SAFETY: the original simulator, occupant, property holder, curve and writable output.
        unsafe {
            let sim = CapacityView::from_ptr(this);
            let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
            let result = property_u32(holder, id, output);
            if result as u8 == 0 {
                write_unaligned(output, 0);
                return result & !0xFF;
            }
            if vcall!(sim.occupants(), 0x7C => fn(*mut u8) -> u32, occupant) as u8 == 0 {
                write_unaligned(output, 0);
            }
            let scaled = multiply_float(unsigned(read_unaligned(output)), sim.efficiency()).ftol();
            write_unaligned(output, scaled as u32);
            let holder_result = virtual_call(occupant, 0x18, scaled as u32, table(occupant), &[]);
            let holder = holder_result as u32 as *mut u8;
            let result = virtual_call(holder, 0x24, table(holder), (holder_result >> 32) as u32, &[0x8A1E_07EE]);
            let property = result as u32 as *mut u8;
            if property.is_null() { return (result & !0xFF) | 1; }
            let mut curve = [0u32; 4];
            curve_new(curve.as_mut_ptr());
            curve_set(curve.as_mut_ptr(), property);
            let mut condition = 0.0;
            let result = property_float(holder, 0xC910_B4CF, &mut condition);
            let result = evaluate(curve.as_mut_ptr(), condition, result as u32, condition.to_bits());
            let product = result.st0 * unsigned(read_unaligned(output));
            let rounded = add_float(product, 12_582_912.0).to_f32().to_bits().wrapping_add(0xB4C0_0000);
            write_unaligned(output, rounded);
            let result = if curve[0] != 0 {
                dispose(curve[0], result.edx)
            } else {
                registers(0, result.edx)
            };
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// Reads the signed 64-bit base cost and condition. Both and a maintenance curve must exist.
    /// Multiplies the exact integer load by the unrounded curve result, truncates to 64 bits, and
    /// frees curve storage. Failures keep the last call's upper EAX bytes and EDX.
    #[port(address = 0x006E_4530, owner = "pollution", name = "cSC4PollutionSimulator::CalcOccupantMaintenanceCost")]
    pub unsafe extern "stdcall" fn fc4p_006e4530(occupant: *mut u8, output: *mut i64) -> u64 {
        // SAFETY: the original occupant, its property holder and the writable 64-bit cost.
        unsafe {
            let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
            write_unaligned(output.cast::<u32>(), 0);
            write_unaligned(output.cast::<u32>().add(1), 0);
            let result = property_i64(holder, 0x2A58_D350, output);
            if result as u8 == 0 { return result & !0xFF; }
            let mut condition = 0.0;
            let result = property_float(holder, 0xC910_B4CF, &mut condition);
            if result as u8 == 0 { return result & !0xFF; }
            let result = virtual_call(holder, 0x24, result as u32, table(holder), &[0x0911_E2E7]);
            let property = result as u32 as *mut u8;
            if property.is_null() { return result & !0xFF; }
            let mut curve = [0u32; 4];
            curve_new(curve.as_mut_ptr());
            let built = curve_set(curve.as_mut_ptr(), property);
            let result = evaluate(curve.as_mut_ptr(), condition, condition.to_bits(), (built >> 32) as u32);
            let cost = (result.st0 * F80::from_i64(read_unaligned(output))).ftol();
            write_unaligned(output, cost);
            let result = if curve[0] != 0 {
                dispose(curve[0], (cost as u64 >> 32) as u32)
            } else {
                registers(0, (cost as u64 >> 32) as u32)
            };
            (result & !0xFF) | 1
        }
    }
}

port! {
    /// For each facility with a cost and budget entry, books the wrapping 64-bit difference when
    /// the stored cost changes, then updates that entry's stored cost. Missing line items skip both.
    #[port(address = 0x006E_4100, owner = "pollution", name = "cSC4PollutionSimulator::CalcMaintenanceCosts")]
    pub unsafe extern "thiscall" fn fc4p_006e4100(this: *mut u8, list: *mut *mut u8) {
        // SAFETY: the original live list, simulator and budget interfaces.
        unsafe {
            let end = read_unaligned(list);
            let mut node = ListNode::from_ptr(end).next();
            while node != end {
                let holder = vcall!(ListNode::from_ptr(node).occupant(), 0x18 => fn() -> *mut u8);
                let mut cost = 0i64;
                if vcall!(this, 0x104 => fn(*mut u8, *mut i64) -> u32, ListNode::from_ptr(node).occupant(), &mut cost) as u8 != 0 {
                    let budget = *budget();
                    let mut key = [0u32; 8];
                    if vcall!(budget, 0xCC => fn(*mut u8, u32, *mut u32) -> u32, holder, 0xCA58_EDC7, key.as_mut_ptr()) as u8 != 0 {
                        let previous = (u64::from(key[4]) | (u64::from(key[5]) << 32)) as i64;
                        if cost != previous {
                            let item = vcall!(budget, 0xE4 => fn(*mut u32) -> *mut u8, key.as_mut_ptr());
                            if !item.is_null() {
                                vcall!(item, 0x34 => fn(i64), cost.wrapping_sub(previous));
                                vcall!(budget, 0xD4 => fn(*mut u8, u32, i64), holder, key[7], cost);
                            }
                        }
                    }
                }
                node = ListNode::from_ptr(node).next();
            }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e4100::PORT, fc4p_006e4300::PORT, fc4p_006e4530::PORT];

#[cfg(test)]
mod tests;
