// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version. This program is distributed WITHOUT ANY WARRANTY; see
// the GNU General Public License for more details. You should have received a
// copy of the license along with this program (see LICENSE at the repository
// root); if not, see <https://www.gnu.org/licenses/>.

//! Plant production and infrastructure accounting.

use super::*;

game_fn! { fn get_uint(holder: *mut u8, id: u32, out: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn get_int64(holder: *mut u8, id: u32, out: *mut u64) -> u32 = "C" 0x005F_D4B0; }

port! {
    /// Read unsigned capacity, then multiply it by the plant's current efficiency using x87 arithmetic.
    /// A failed property lookup returns the helper's output word even if it was changed.
    #[port(address = 0x006E_B1C0, owner = "power", name = "cSC4PowerSimulator::GetCurrentPowerProduced")]
    pub unsafe extern "thiscall" fn fc4p_006eb1c0(this: *mut u8, plant: *mut u8) -> u32 {
        unsafe {
            let mut capacity = 0u32;
            if plant.is_null() { return capacity; }
            let holder = vcall!(plant, 0x18 => fn() -> *mut u8);
            if get_uint(holder, 0x2781_2852, &mut capacity) as u8 == 0 { return capacity; }
            let mut temporary = core::mem::MaybeUninit::<f32>::uninit();
            let efficiency = vcall!(this, 0xB4 => fn(*mut f32, *mut u8) -> *mut f32, temporary.as_mut_ptr(), plant);
            let mut value = F80::from_i32(capacity as i32);
            if (capacity as i32) < 0 { value = add_float(value, 4_294_967_296.0); }
            multiply_float(value, efficiency.read_unaligned()).ftol() as u32
        }
    }
}
port! {
    /// Sum the low words of pole costs with wrapping, clear the high cost word, and report whether it changed.
    /// The upper EAX bytes retain the property reader, old high cost, or entry EAX on the empty path.
    #[port(address = 0x006E_B490, owner = "power", name = "cSC4PowerSimulator::RecomputeInfrastructureCosts", entry_eax)]
    pub unsafe extern "thiscall" fn fc4p_006eb490(this: *mut u8, [entry_eax: u32]) -> u32 {
        unsafe {
            let sim = PowerSim::from_ptr(this);
            let (mut cursor, end) = (sim.poles_begin(), sim.poles_end());
            let mut total = 0u32;
            let mut eax = entry_eax;
            while cursor != end {
                let pole = cursor.read_unaligned();
                let occupant = vcall!(pole, 0x0C => fn() -> *mut u8);
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                let mut cost = 0u64;
                eax = get_int64(holder, 0x0913_2DE8, &mut cost);
                total = total.wrapping_add(cost as u32);
                cursor = cursor.wrapping_add(1);
            }
            let old = sim.infrastructure_cost();
            let changed = if old as u32 != total { true } else {
                eax = (old >> 32) as u32;
                eax != 0
            };
            sim.set_infrastructure_cost(u64::from(total));
            (eax & !255) | u32::from(changed)
        }
    }
}

game_data! { static budget_simulator: *mut u8 = 0x00B4_3D64; }
game_data! { static zero_percent: u32 = 0x00B4_A5B0; }
game_fn! { fn curve_output(curve:*mut u8, x:f32) -> St0 = "thiscall" 0x005F_4250; }
game_fn! { fn production(this:*mut u8, plant:*mut u8) -> u32 = "thiscall" 0x006E_B1C0; }
game_fn! { fn get_int(holder:*mut u8, id:u32, out:*mut i32) -> u32 = "C" 0x005F_D450; }
game_fn! { fn set_int(holder:*mut u8, id:u32, value:i32) -> u32 = "C" 0x005F_E330; }

layout! {
    /// Links and payload of the circular plant list.
    struct PlantNode {
        0x00 => next / set_next: *mut u8,
        0x08 => visited / set_visited: u32,
        0x0C => occupant / set_occupant: *mut u8,
    }
}

unsafe extern "fastcall" fn efficiency_body(this: *mut u8, entry_edx: u32, args: *mut u32) -> u64 {
    unsafe {
        let plant = args.add(1).read() as *mut u8;
        let zero = *zero_percent();
        if plant.is_null() {
            let out = args.read() as *mut u32;
            out.write_unaligned(zero);
            return (u64::from(entry_edx) << 32) | u64::from(out as u32);
        }
        let budget = *budget_simulator();
        // The budget slot is selected before querying the occupant's holder.
        let slot = crate::abi::vslot(budget, 0xCC);
        let query =
            core::mem::transmute::<usize, unsafe extern "thiscall" fn(*mut u8, *mut u8, u32, *mut u32) -> u32>(slot);
        let mut record = core::mem::MaybeUninit::<[u32; 8]>::uninit();
        let record_ptr = record.as_mut_ptr().cast::<u32>();
        record_ptr.add(7).write(0);
        let holder = vcall!(plant,0x18=>fn()->*mut u8);
        let mut fraction = f32::from_bits(zero);
        if query(budget, holder, 0xCA58_E540, record_ptr) as u8 != 0 {
            let funding = f32::from_bits(record_ptr.add(7).read());
            let x = multiply_float(F80::from_f32(funding), 100.0).to_f32();
            let value = curve_output(this.add(0x378), x);
            fraction = multiply_float(value, 0.01).to_f32();
        }
        // The caller's plant argument is the condition method's scratch output word.
        let condition = vcall!(this,0xB8=>fn(*mut f32,*mut u8)->*mut f32,args.add(1).cast(),plant);
        let bits = multiply_float(F80::from_f32(fraction), condition.read_unaligned())
            .to_f32()
            .to_bits();
        let out = args.read() as *mut u32;
        out.write_unaligned(bits);
        (u64::from(bits) << 32) | u64::from(out as u32)
    }
}
port! {
    /// Evaluate the funding curve, store its fraction, then multiply by current condition.
    /// The plant argument word is scratch output for the condition query; EDX returns the result bits.
    #[port(address = 0x006E_B5F0, owner = "power", name = "cSC4PowerSimulator::GetCurrentPlantEfficiencyPercent", naked)]
    pub unsafe extern "C" fn fc4p_006eb5f0() {
        core::arch::naked_asm!("lea eax, [esp + 4]","push eax","call {body}","ret 8",body=sym efficiency_body);
    }
}

unsafe extern "thiscall" fn visited_body(this: *mut u8, args: *mut u32) -> u32 {
    unsafe {
        let sim = PowerSim::from_ptr(this);
        if sim.production() > 0 {
            let numerator = args.read();
            let denominator = args.add(1).read();
            let mut a = F80::from_i32(numerator as i32);
            if (numerator as i32) < 0 {
                a = add_float(a, 4_294_967_296.0);
            }
            let mut b = F80::from_i32(denominator as i32);
            if (denominator as i32) < 0 {
                b = add_float(b, 4_294_967_296.0);
            }
            args.write((a / b).to_f32().to_bits());
        } else {
            args.write(0);
        }
        let header = sim.plant_header();
        let mut cursor = PlantNode::from_ptr(header).next();
        while cursor != header {
            let node = PlantNode::from_ptr(cursor);
            if node.visited() == 1 {
                let mut kind = 0i32;
                let holder = vcall!(node.occupant(),0x18=>fn()->*mut u8);
                let produced = production(this, node.occupant());
                get_int(holder, 0x2781_2853, &mut kind);
                let used = multiply_float(F80::from_i32(produced as i32), f32::from_bits(args.read())).ftol() as i32;
                set_int(holder, 0x08D9_763D, used);
                let index = sim.ring_index().wrapping_mul(10).wrapping_add(kind);
                let cell = this
                    .wrapping_offset(0x90)
                    .wrapping_offset(index.wrapping_mul(4) as isize)
                    .cast::<i32>();
                cell.write_unaligned(cell.read_unaligned().wrapping_add(used));
                node.set_visited(2);
            }
            cursor = node.next();
        }
        header as u32
    }
}
port! {
    /// Record signed, truncated plant usage in its property and ring row, then change visited state 1 to 2.
    /// The numerator argument is replaced by the stored unsigned production ratio, or zero when inactive.
    #[port(address = 0x006E_BC10, owner = "power", name = "cSC4PowerSimulator::MarkPowerPlantsAsVisited", naked)]
    pub unsafe extern "C" fn fc4p_006ebc10() {
        core::arch::naked_asm!("lea eax, [esp + 4]","push eax","call {body}","ret 8",body=sym visited_body);
    }
}

game_fn! { fn curve_new(this:*mut u8) -> u32 = "thiscall" 0x007D_0410; }
game_fn! { fn curve_set(this:*mut u8, property:*mut u8) -> u32 = "thiscall" 0x005F_4620; }
game_fn! { fn get_float(holder:*mut u8, id:u32, out:*mut u32) -> u64 = "C" 0x005F_D4F0; }
game_fn! { fn free_curve_storage(storage:*mut u8) -> u32 = "C" 0x0090_CF63; }

unsafe extern "fastcall" fn condition_body(_this: *mut u8, entry_edx: u32, args: *mut u32) -> u64 {
    use crate::abi::{call_raw, vslot, Conv, RawCall};
    unsafe {
        let plant = args.add(1).read() as *mut u8;
        let zero = *zero_percent();
        let mut bits = zero;
        let mut edx = entry_edx;
        if !plant.is_null() {
            let holder = vcall!(plant,0x18=>fn()->*mut u8);
            let id = [0x8A1E_07EE];
            let mut property = RawCall::new(Conv::Thiscall, vslot(holder, 0x24), holder as u32, &id);
            property.eax = holder as u32;
            property.edx = holder.cast::<u32>().read_unaligned();
            call_raw(&mut property);
            edx = property.edx;
            if property.eax != 0 {
                let mut curve = core::mem::MaybeUninit::<[u32; 4]>::uninit();
                let cp = curve.as_mut_ptr().cast::<u8>();
                curve_new(cp);
                curve_set(cp, property.eax as *mut u8);
                args.add(1).write(0);
                let read = get_float(holder, 0xC910_B4CF, args.add(1));
                let x = [args.add(1).read()];
                let mut output = RawCall::new(Conv::Thiscall, crate::abi::target(0x005F_4250), cp as u32, &x);
                output.eax = read as u32;
                output.edx = (read >> 32) as u32;
                output.take_st0 = 1;
                call_raw(&mut output);
                bits = output.st0.to_f32().to_bits();
                edx = output.edx;
                let storage = cp.cast::<*mut u8>().read_unaligned();
                if !storage.is_null() {
                    free_curve_storage(storage);
                    edx = bits;
                }
            }
        }
        let out = args.read() as *mut u32;
        out.write_unaligned(bits);
        (u64::from(edx) << 32) | u64::from(out as u32)
    }
}
port! {
    /// Build the occupant's condition curve, evaluate it at its stored wear, round to a float, and free its data.
    /// The plant argument word holds wear. EDX is retained from the property/curve call unless storage was freed.
    #[port(address = 0x006E_C820, owner = "power", name = "cSC4PowerSimulator::GetCurrentPlantConditionPercent", naked)]
    pub unsafe extern "C" fn fc4p_006ec820() {
        core::arch::naked_asm!("lea eax, [esp + 4]","push eax","call {body}","ret 8",body=sym condition_body);
    }
}

pub const PORTS: &[Port] = &[
    fc4p_006ec820::PORT,
    fc4p_006eb5f0::PORT,
    fc4p_006ebc10::PORT,
    fc4p_006eb1c0::PORT,
    fc4p_006eb490::PORT,
];

#[cfg(test)]
mod tests;
