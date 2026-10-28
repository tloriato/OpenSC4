// SPDX-License-Identifier: GPL-3.0-or-later
//! Completed-pass statistics, histories, map notifications and sanitation efficiency.

// One shared zero-count branch skips all four accumulator reads and divisions.
#![allow(clippy::manual_checked_ops)]

use super::*;

game_fn! { fn recycling(this: *mut u8) = "thiscall" 0x006E_6DA0; }
game_fn! { fn post(id: u32, data1: u32, data2: u32, data3: u32, data4: u32, unknown: u32) = "C" 0x005F_98C0; }
game_fn! { fn curve_new(curve: *mut u32) = "thiscall" 0x007D_0410; }
game_fn! { fn curve_load(curve: *mut u32, resource: *const u32, property: u32) = "thiscall" 0x005F_4700; }
game_fn! { fn curve_output(curve: *mut u32, input: f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn curve_free(storage: u32) = "C" 0x0090_CF63; }
game_data! { static history: *mut u8 = 0x00B4_3DC0; }
game_data! { static percent: f32 = 0x00A8_0AA0; }
game_data! { static fraction: f32 = 0x00A8_23A4; }

layout! {
    /// The statistics accumulated during one pollution pass.
    struct CycleView {
        0xD8 => department / set_department: *mut u8,
        0x108 => air_sum / set_air_sum: u32,
        0x10C => treated_water_sum / set_treated_water_sum: u32,
        0x110 => water_sum / set_water_sum: u32,
        0x114 => treatment_sum / set_treatment_sum: u32,
        0x118 => previous_water_sum / set_previous_water_sum: u32,
        0x11C => previous_treatment_sum / set_previous_treatment_sum: u32,
        0x120 => developed / set_developed: u32,
        0x124 => radiation_sum / set_radiation_sum: u32,
        0x138 => garbage_uncollected / set_garbage_uncollected: u8,
        0x208 => dirty_radiation / set_dirty_radiation: u8,
        0x228 => efficiency / set_efficiency: f32,
    }
}

/// Multiplies a register by the stored float without first quieting a signaling NaN in that operand.
fn multiply_float(value: F80, operand: f32) -> F80 {
    let input = value.to_bytes();
    let mut output = [0u8; 10];
    // SAFETY: both operands and the destination are live; the x87 stack is balanced.
    unsafe {
        core::arch::asm!("fld tbyte ptr [{input}]", "fmul dword ptr [{operand}]", "fstp tbyte ptr [{output}]",
            input = in(reg) &input, operand = in(reg) &operand, output = in(reg) &mut output,
            out("st(0)") _);
    }
    F80::from_bytes(output)
}

port! {
    /// Completes the pass using unsigned averages, saves the water totals, records five histories,
    /// resets the accumulators, posts each dirty map in order, and clears the four flags. Finally
    /// reloads the funding curve, stores efficiency, posts deal status and releases curve storage.
    #[port(address = 0x006E_7930, owner = "pollution", name = "cSC4PollutionSimulator::EndOfCycle")]
    pub unsafe extern "thiscall" fn fc4p_006e7930(this: *mut u8) {
        // SAFETY: the simulator, its services and globals have their original game layouts.
        unsafe {
            let sim = PollutionSim::from_ptr(this);
            let cycle = CycleView::from_ptr(this);
            recycling(this);
            let uncollected = sim.uncollected();
            let developed = cycle.developed();
            cycle.set_garbage_uncollected(u8::from(uncollected != 0));
            let mut radiation_percent = 0;
            if developed != 0 {
                sim.set_average_air(cycle.air_sum() / developed);
                sim.set_average_water((cycle.treated_water_sum() / developed) as i32);
                sim.set_average_garbage((uncollected / developed) as u16);
                radiation_percent = cycle.radiation_sum().wrapping_mul(100) / developed;
            } else {
                sim.set_average_air(0);
                sim.set_average_water(0);
                sim.set_average_garbage(0);
            }
            cycle.set_previous_water_sum(cycle.water_sum());
            cycle.set_previous_treatment_sum(cycle.treatment_sum());
            let history = *history();
            vcall!(history, 0x30 => fn(u32, u32), 0x0A11_00E8, sim.average_air());
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0130, sim.average_water() as u32);
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0134, u32::from(sim.average_garbage()));
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0137, cycle.radiation_sum());
            vcall!(history, 0x30 => fn(u32, u32), 0xEA11_0138, sim.average_air().wrapping_add(radiation_percent));
            let dirty_radiation = cycle.dirty_radiation();
            cycle.set_radiation_sum(0);
            cycle.set_developed(0);
            cycle.set_treatment_sum(0);
            cycle.set_water_sum(0);
            cycle.set_treated_water_sum(0);
            cycle.set_air_sum(0);
            if dirty_radiation != 0 { post(0xDA6D_D35D, 0, 0, 0, 0, 0); }
            if sim.dirty_air() != 0 { post(0xFA6D_D0BB, 0, 0, 0, 0, 0); }
            if sim.dirty_water() != 0 { post(0x5A6D_D1AA, 0, 0, 0, 0, 0); }
            if sim.dirty_garbage() != 0 { post(0xDA6D_D2CB, 0, 0, 0, 0, 0); }
            cycle.set_dirty_radiation(0);
            sim.set_dirty_air(0);
            sim.set_dirty_water(0);
            sim.set_dirty_garbage(0);
            let mut curve = [0u32; 4];
            curve_new(curve.as_mut_ptr());
            let resource = [0x6534_284A, 0xE7E2_C2DB, 0xC911_E35B];
            curve_load(curve.as_mut_ptr(), resource.as_ptr(), 0x6911_E2DF);
            let funding = vcall!(cycle.department(), 0x28 => fn() -> *const f32);
            let input = multiply_float(F80::from_f32(read_unaligned(funding)), *percent()).to_f32();
            let output = curve_output(curve.as_mut_ptr(), input);
            cycle.set_efficiency(multiply_float(output, *fraction()).to_f32());
            post(0xEA41_8D8F, 8, 0, 0, 0, 0);
            if curve[0] != 0 { curve_free(curve[0]); }
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e7930::PORT];

#[cfg(test)]
mod tests;
