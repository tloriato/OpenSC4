// SPDX-License-Identifier: GPL-3.0-or-later
//! Daily capacity queries, neighbor deals, pollution visits and facility usage.

use super::*;
use core::mem::MaybeUninit;

game_fn! { fn property_u32(holder: *mut u8, id: u32, output: *mut u32) -> u32 = "C" 0x005F_D480; }
game_fn! { fn dispose(this: *mut u8, offered: f32, incinerators: *mut u32, energy: *mut u32) -> St0 = "thiscall" 0x006E_3820; }
game_fn! { fn distribute(this: *mut u8, list: *mut u8, capacity: u32, used: u32, total: u32, amount: *mut u32, count: u32) = "thiscall" 0x006E_3FE0; }
game_fn! { fn maintenance(this: *mut u8, list: *mut u8) = "thiscall" 0x006E_4100; }
game_fn! { fn reset(iterator: *mut u8, shuffle: u32) = "thiscall" 0x0049_D1C0; }
game_fn! { fn next(iterator: *mut u8, x: *mut i32, z: *mut i32) = "thiscall" 0x0049_CB10; }
game_fn! { fn end_cycle(this: *mut u8) = "thiscall" 0x006E_7930; }
game_fn! { fn framework() -> *mut u8 = "C" 0x0087_93EC; }
game_fn! { fn release_allocator(pointer: *mut *mut u8, edx: u32) -> u64 = "fastcall" 0x0041_1870; }
game_fn! { fn clear_deals(list: *mut *mut u8) -> u64 = "thiscall" 0x0042_1660; }
game_data! { static allocator: *mut u8 = 0x00B2_0D58; }
game_data! { static zero: f32 = 0x00A8_1054; }
game_data! { static one: f32 = 0x00A8_1228; }
game_data! { static tenth: f32 = 0x00AB_2550; }
game_data! { static bias: f32 = 0x00AB_2554; }
game_data! { static maximum: f32 = 0x00AB_274C; }
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static traffic_scale: f32 = 0x00B4_9B8C; }
game_data! { static recycling_threshold: f32 = 0x00B4_9BC4; }
game_data! { static air_delta: u32 = 0x00B4_9B88; }
game_data! { static water_delta: u32 = 0x00B4_9B84; }
game_data! { static valve_maximum: u32 = 0x00B4_9B68; }

layout! {
    /// Daily services, scan coordinates and accumulated counters.
    struct DailyView {
        0x20 => ordinances / set_ordinances: *mut u8,
        0x28 => traffic / set_traffic: *mut u8,
        0x30 => neighbors / set_neighbors: *mut u8,
        0x60 => x / set_x: i32,
        0x64 => z / set_z: i32,
        0x108 => air_sum / set_air_sum: i32,
        0x10C => treated_water / set_treated_water: i32,
        0x110 => water_sum / set_water_sum: i32,
        0x114 => treatment_sum / set_treatment_sum: i32,
        0x118 => previous_water / set_previous_water: i32,
        0x11C => previous_treated / set_previous_treated: i32,
        0x120 => developed / set_developed: u32,
        0x124 => radiation_sum / set_radiation_sum: i32,
        0x12C => incinerator_capacity / set_incinerator_capacity: u32,
        0x130 => energy_capacity / set_energy_capacity: u32,
        0x140 => produced / set_produced: i64,
        0x148 => imported / set_imported: i64,
        0x158 => recycled / set_recycled: f64,
        0x1B0 => export_capacity / set_export_capacity: i32,
        0x1D4 => visits / set_visits: u32,
        0x1D8 => iterator_state / set_iterator_state: u8,
    }
}

macro_rules! stored_op {
    ($name:ident, $ty:ty, $instruction:literal) => {
        fn $name(value: F80, operand: $ty) -> F80 {
            let input = value.to_bytes(); let mut output = [0u8; 10];
            // SAFETY: live operands, one x87 push and pop.
            unsafe {
                core::arch::asm!("fld tbyte ptr [{input}]", $instruction, "fstp tbyte ptr [{output}]",
                    input = in(reg) &input, operand = in(reg) &operand,
                    output = in(reg) &mut output, out("st(0)") _);
            }
            F80::from_bytes(output)
        }
    };
}
stored_op!(mul_float, f32, "fmul dword ptr [{operand}]");
stored_op!(add_float, f32, "fadd dword ptr [{operand}]");
stored_op!(sub_float, f32, "fsub dword ptr [{operand}]");
stored_op!(sub_from_float, f32, "fsubr dword ptr [{operand}]");
stored_op!(div_float, f32, "fdiv dword ptr [{operand}]");
stored_op!(div_from_float, f32, "fdivr dword ptr [{operand}]");
stored_op!(add_integer, i32, "fiadd dword ptr [{operand}]");
stored_op!(mul_integer, i32, "fimul dword ptr [{operand}]");
stored_op!(div_integer, i32, "fidiv dword ptr [{operand}]");
stored_op!(add_double, f64, "fadd qword ptr [{operand}]");

fn greater(value: F80, operand: f32) -> bool {
    let c = value.compare(F80::from_f32(operand));
    !c.less() && !c.equal() && !c.unordered()
}
unsafe fn rounded(value: F80) -> i32 {
    // SAFETY: the game's stored rounding bias.
    add_float(value, unsafe { *bias() })
        .to_f32()
        .to_bits()
        .wrapping_add(0xB4C0_0000) as i32
}
unsafe fn unsigned(value: u32) -> F80 {
    let signed = F80::from_i32(value as i32);
    if value as i32 >= 0 {
        signed
    } else {
        // SAFETY: the game's exact 2^32 float.
        add_float(signed, unsafe { *unsigned_bias() })
    }
}

/// Resolves the allocator and preserves the EDX left by acquiring its interface.
unsafe fn get_allocator(mut edx: u32) -> (*mut u8, u32) {
    // SAFETY: the allocator global and framework's allocator factory.
    unsafe {
        let mut current = *allocator();
        if current.is_null() {
            let mut acquired = core::ptr::null_mut();
            let framework = framework();
            if !framework.is_null() {
                vcall!(framework, 0x14 => fn(u32, u32, *mut *mut u8), 0x3AE4_BEA3, 0x3AE4_BEAB, &mut acquired);
            }
            *allocator() = acquired;
            edx = (release_allocator(&mut acquired, acquired as u32) >> 32) as u32;
            current = *allocator();
        }
        (current, edx)
    }
}

/// Repeated queries are intentional: capacity callbacks may return different values on each call.
unsafe fn capacity_list(
    this: *mut u8,
    offset: usize,
    capacity: u32,
    used_id: u32,
    total_offset: Option<usize>,
    output: &mut MaybeUninit<u32>,
    unused_output: &mut MaybeUninit<u32>,
) -> (u32, u32) {
    // SAFETY: original circular lists, virtual capacity query and property helper.
    unsafe {
        let end = read_unaligned(this.add(offset).cast::<*mut u8>());
        if let Some(offset) = total_offset {
            write_unaligned(this.add(offset).cast::<u32>(), 0);
        }
        let mut node = ListNode::from_ptr(end).next();
        let mut remaining = 0u32;
        let mut count = 0u32;
        while node != end {
            if let Some(offset) = total_offset {
                let occupant = ListNode::from_ptr(node).occupant();
                if vcall!(this, 0xD4 => fn(u32, *mut u8, *mut u32) -> u32,
                    capacity, occupant, output.as_mut_ptr()) as u8
                    != 0
                {
                    let total = read_unaligned(this.add(offset).cast::<u32>());
                    write_unaligned(this.add(offset).cast::<u32>(), total.wrapping_add(output.assume_init()));
                }
            }
            let occupant = ListNode::from_ptr(node).occupant();
            let mut used = 0u32;
            let out = if total_offset.is_some() {
                unused_output.as_mut_ptr()
            } else {
                output.as_mut_ptr()
            };
            if vcall!(this, 0xD4 => fn(u32, *mut u8, *mut u32) -> u32, capacity, occupant, out) as u8 != 0 {
                let holder = vcall!(occupant, 0x18 => fn() -> *mut u8);
                if property_u32(holder, used_id, &mut used) as u8 != 0 {
                    remaining = remaining.wrapping_add(read_unaligned(out).saturating_sub(used));
                }
            }
            node = ListNode::from_ptr(node).next();
            count = count.wrapping_add(1);
        }
        (remaining, count)
    }
}

/// Calls through an already captured table, including when the preceding getter changes the object table.
unsafe fn saved_call(object: *mut u8, functions: u32, slot: usize, args: &[u32]) -> u32 {
    // SAFETY: the table was read from this live object's original virtual interface.
    unsafe {
        let target = read_unaligned((functions as *const u8).add(slot).cast::<u32>());
        let mut call = RawCall::new(Conv::Thiscall, target as usize, object as u32, args);
        call_raw(&mut call);
        call.eax
    }
}

struct Factors {
    traffic: f32,
    city: [f32; 3],
    zones: [[f32; 3]; 16],
}

unsafe fn factors(sim: DailyView, output: *mut Factors) {
    // SAFETY: each factor call returns ST0, which the game stores to a float before the next call.
    unsafe {
        core::ptr::addr_of_mut!((*output).traffic)
            .write(vcall_st0!(sim.ordinances(), 0x64, 0x8A67_E378u32, 0u32).to_f32());
        for (i, id) in [0x08F7_9B8Eu32, 0xE8F7_9C8B, 0xE8F7_9C90].into_iter().enumerate() {
            core::ptr::addr_of_mut!((*output).city[i]).write(vcall_st0!(sim.ordinances(), 0x64, id, 0u32).to_f32());
        }
        for zone in 0..16 {
            for (i, id) in [0x8A67_E373u32, 0x8A67_E374, 0x8A67_E376].into_iter().enumerate() {
                core::ptr::addr_of_mut!((*output).zones[zone][i])
                    .write(vcall_st0!(sim.ordinances(), 0x64, id, zone as u32).to_f32());
            }
        }
    }
}

/// Applies cell garbage, recycling and valve backflow, retaining the register precision until each store.
#[allow(clippy::too_many_arguments)]
unsafe fn garbage(
    this: *mut u8,
    sim: DailyView,
    x: i32,
    z: i32,
    previous: i32,
    generated: i16,
    count: i32,
    incinerators: &mut u32,
    energy: &mut u32,
) {
    // SAFETY: original fields, constants, virtual setters and disposal entry point.
    unsafe {
        let common = PollutionSim::from_ptr(this);
        let generated_float = F80::from_i32(i32::from(generated)).to_f32();
        let count_float = F80::from_i32(count).to_f32();
        let mut share = div_float(F80::from_f32(generated_float), count_float);
        sim.set_produced(sim.produced().wrapping_add(i64::from(rounded(share))));
        if greater(F80::from_f32(common.recycling_effect()), *recycling_threshold()) && greater(share, *zero()) {
            let mut recycled = mul_float(share, common.recycling_effect());
            let compared = recycled.compare(share);
            if !compared.less() && !compared.equal() && !compared.unordered() {
                recycled = share;
            }
            sim.set_recycled(add_double(recycled, sim.recycled()).to_f64());
            share = share - recycled;
        }
        let total = (F80::from_i32(previous) + share).to_f32();
        vcall!(this, 0x70 => fn(i32, i32, i32), x, z, 0);
        let uncollected = common.uncollected();
        if uncollected >= previous as u32 {
            common.set_uncollected(uncollected.wrapping_sub(previous as u32));
        }
        let left = sub_from_float(dispose(this, total, incinerators, energy), total);
        let mut stored_left = left.to_f32();
        if !greater(left, *zero()) {
            return;
        }
        if greater(F80::from_f32(stored_left), *maximum()) {
            stored_left = sub_float(F80::from_f32(stored_left), *maximum()).to_f32();
            vcall!(this, 0x70 => fn(i32, i32, i32), x, z, 32767);
            common.set_uncollected(common.uncollected().wrapping_add(32767));
            if greater(F80::from_f32(stored_left), *zero()) {
                vcall!(this, 0x74 => fn(i32), F80::from_f32(stored_left).ftol() as i32);
            }
            return;
        }
        let room = common.capacity().wrapping_sub(common.supply());
        let below_max = F80::from_f32(stored_left).compare(F80::from_f32(*maximum())).less();
        if room >= 0 || !below_max {
            vcall!(this, 0x70 => fn(i32, i32, i32), x, z, F80::from_f32(stored_left).ftol() as i32);
            common.set_uncollected(add_float(unsigned(common.uncollected()), stored_left).ftol() as u32);
        } else {
            let mut take = room.wrapping_neg();
            let per_cell = rounded(div_float(F80::from_f32(generated_float), count_float));
            if take > per_cell {
                take = if generated == 0 { 1 } else { per_cell };
            }
            if greater(add_float(F80::from_i32(take), stored_left), *maximum()) {
                take = sub_float(F80::from_f32(*maximum()), stored_left).ftol() as i32;
            }
            vcall!(this, 0x74 => fn(i32), take.wrapping_neg());
            let take_float = F80::from_i32(take).to_f32();
            let value = add_float(F80::from_f32(take_float), stored_left).ftol() as i32;
            vcall!(this, 0x70 => fn(i32, i32, i32), x, z, value);
            let total = add_float(add_float(unsigned(common.uncollected()), take_float), stored_left);
            common.set_uncollected(total.ftol() as u32);
        }
    }
}

/// Reads water neighbors only while none exceeds one, but always queries both source-grid dimensions.
unsafe fn water_neighbor_hot(this: *mut u8, sim: DailyView) -> bool {
    // SAFETY: original coordinates and virtual grid getters.
    unsafe {
        let common = PollutionSim::from_ptr(this);
        let mut value = MaybeUninit::<i32>::uninit();
        let mut cool = true;
        if sim.x() != 0 {
            vcall!(this, 0x20 => fn(i32, i32, *mut i32), sim.x().wrapping_sub(1), sim.z(), value.as_mut_ptr());
            cool = value.assume_init() <= 1;
        }
        if sim.z() != 0 && cool {
            vcall!(this, 0x20 => fn(i32, i32, *mut i32), sim.x(), sim.z().wrapping_sub(1), value.as_mut_ptr());
            cool = value.assume_init() <= 1;
        }
        let x = sim.x();
        let width = vcall!(common.air_source(), 0x30 => fn() -> u32).wrapping_sub(1);
        if (x as u32) < width && cool {
            vcall!(this, 0x20 => fn(i32, i32, *mut i32), x.wrapping_add(1), sim.z(), value.as_mut_ptr());
            cool = value.assume_init() <= 1;
        }
        let z = sim.z();
        let height = vcall!(common.air_source(), 0x34 => fn() -> u32).wrapping_sub(1);
        if (z as u32) < height && cool {
            vcall!(this, 0x20 => fn(i32, i32, *mut i32), sim.x(), z.wrapping_add(1), value.as_mut_ptr());
            cool = value.assume_init() <= 1;
        }
        !cool
    }
}

unsafe fn visit(
    this: *mut u8,
    sim: DailyView,
    factors: &Factors,
    treatment: &mut u32,
    incinerators: &mut u32,
    energy: &mut u32,
) {
    // SAFETY: live native services and valid zone types in 0..16.
    unsafe {
        let common = PollutionSim::from_ptr(this);
        let mut source = [0i16; 4];
        for (i, offset) in [0x40usize, 0x44, 0x48, 0x4C].into_iter().enumerate() {
            let grid = read_unaligned(this.add(offset).cast::<*mut u8>());
            source[i] = vcall!(grid, 0x1C => fn(i32, i32) -> i32, sim.x(), sim.z()) as i16;
            if source[i] < 0 {
                source[i] = 0;
            }
        }
        let mut scaled = [0i32; 3];
        for i in 0..3 {
            scaled[i] = mul_float(F80::from_i32(i32::from(source[i])), factors.city[i]).ftol() as i32;
        }
        let mut sums = [0.0f32; 3];
        let mut count = 0i32;
        let mut developed = false;
        for cell in 0..4i32 {
            let x = sim.x().wrapping_add(cell & 1);
            let z = sim.z().wrapping_add(cell >> 1);
            let zone = vcall!(common.zones(), 0x30 => fn(i32, i32) -> u32, x, z);
            if zone != 14 {
                let values = &*factors.zones.as_ptr().wrapping_add(zone as usize);
                for i in 0..3 {
                    sums[i] = add_float(F80::from_f32(sums[i]), values[i]).to_f32();
                }
                count += 1;
                developed |= zone != 0;
            }
        }
        if developed {
            sim.set_developed(sim.developed().wrapping_add(1));
        }
        if count > 0 {
            let inverse = div_from_float(F80::from_i32(count), *one());
            let air_factor = F80::from_f32(sums[0]) * inverse;
            sums[1] = mul_float(inverse, sums[1]).to_f32();
            sums[2] = mul_float(inverse, sums[2]).to_f32();
            scaled[0] = (F80::from_i32(i32::from(scaled[0] as i16)) * air_factor).ftol() as i32;
            scaled[1] = mul_float(F80::from_i32(i32::from(scaled[1] as i16)), sums[1]).ftol() as i32;
            scaled[2] = mul_float(F80::from_i32(i32::from(scaled[2] as i16)), sums[2]).ftol() as i32;
        }
        let mut air = i32::from(scaled[0] as i16);
        let mut water = i32::from(scaled[1] as i16);
        let mut previous = MaybeUninit::<i32>::uninit();
        for cell in 0..4i32 {
            let x = sim.x().wrapping_add(cell & 1);
            let z = sim.z().wrapping_add(cell >> 1);
            let landfill = vcall!(common.zones(), 0x30 => fn(i32, i32) -> u32, x, z) == 14;
            vcall!(this, 0x24 => fn(i32, i32, *mut i32), x, z, previous.as_mut_ptr());
            let traffic = vcall!(sim.traffic(), 0x20 => fn() -> *mut u8);
            let traffic = vcall!(traffic, 0x1C => fn(i32, i32) -> u32, x, z) as u8;
            let contribution = mul_float(
                mul_float(F80::from_i32(i32::from(traffic)), factors.traffic),
                *traffic_scale(),
            );
            air = add_integer(contribution, air).ftol() as i32;
            if !landfill {
                garbage(
                    this,
                    sim,
                    x,
                    z,
                    previous.assume_init(),
                    scaled[2] as i16,
                    count,
                    incinerators,
                    energy,
                );
            }
        }
        sim.set_water_sum(sim.water_sum().wrapping_add(water));
        if *treatment != 0 && water > 0 {
            let mut treated = *treatment as i32;
            let previous_water = sim.previous_water();
            let previous_treated = sim.previous_treated();
            if previous_water > 0 && previous_treated > 0 && *treatment <= previous_treated as u32 {
                let ratio = div_integer(F80::from_i32(previous_treated), previous_water);
                treated = rounded(mul_integer(ratio, water));
                if treated == 0 {
                    treated = 1;
                }
            }
            treated = treated.min(water);
            let treated = (treated as u32).min(*treatment);
            sim.set_treatment_sum(sim.treatment_sum().wrapping_add(treated as i32));
            water = water.wrapping_sub(treated as i32);
            *treatment = treatment.wrapping_sub(treated);
        }
        sim.set_treated_water(sim.treated_water().wrapping_add(water));
        sim.set_air_sum(sim.air_sum().wrapping_add(air));
        sim.set_radiation_sum(sim.radiation_sum().wrapping_add(i32::from(source[3])));
        let air = air.clamp(0, 32767);
        let water = water.clamp(0, 32767);
        let mut current = MaybeUninit::<i32>::uninit();
        vcall!(this, 0x1C => fn(i32, i32, *mut i32), sim.x(), sim.z(), current.as_mut_ptr());
        let old = current.assume_init();
        if old != air {
            let value = if old < air {
                old.wrapping_add((air.wrapping_sub(old) as u32).min(*air_delta()) as i32)
            } else {
                old.wrapping_sub((old.wrapping_sub(air) as u32).min(*air_delta()) as i32)
            };
            vcall!(this, 0x68 => fn(i32, i32, i32), sim.x(), sim.z(), value);
        }
        vcall!(this, 0x20 => fn(i32, i32, *mut i32), sim.x(), sim.z(), current.as_mut_ptr());
        let old = current.assume_init();
        if old != water {
            let mut value = if old < water {
                old.wrapping_add((water.wrapping_sub(old) as u32).min(*water_delta()) as i32)
            } else {
                old.wrapping_sub((old.wrapping_sub(water) as u32).min(*water_delta()) as i32)
            };
            if old > water && value == 0 && water_neighbor_hot(this, sim) {
                value = 1;
            }
            vcall!(this, 0x6C => fn(i32, i32, i32), sim.x(), sim.z(), value);
        }
        let iterator = this.add(0x1D8);
        if sim.iterator_state() == 2 {
            reset(iterator, 1);
            end_cycle(this);
        }
        let mut x = MaybeUninit::<i32>::uninit();
        let mut z = MaybeUninit::<i32>::uninit();
        next(iterator, x.as_mut_ptr(), z.as_mut_ptr());
        sim.set_x(x.assume_init().wrapping_mul(2));
        sim.set_z(z.assume_init().wrapping_mul(2));
    }
}

port! {
    /// Recomputes capacities, processes deals, visits the configured number of tracts, drains the valve,
    /// distributes used capacity and books facility maintenance. Every helper is called at its game address.
    /// The temporary deal-list allocator's final free supplies both integer result words.
    #[port(address = 0x006E_7FD0, owner = "pollution", name = "cSC4PollutionSimulator::DoDailySimulation")]
    pub unsafe extern "thiscall" fn fc4p_006e7fd0(this: *mut u8) -> u64 {
        // SAFETY: original simulator, circular lists, services and writable call outputs.
        unsafe {
            let sim = DailyView::from_ptr(this); let common = PollutionSim::from_ptr(this);
            let mut output = MaybeUninit::<u32>::uninit(); let mut unused = MaybeUninit::<u32>::uninit();
            let (initial_treatment, treatment_count) = capacity_list(this, 0x68, 0xC8EB_D131, 0x28EC_F510, None, &mut output, &mut unused);
            let (initial_incinerators, incinerator_count) = capacity_list(this, 0x6C, 0xA8EB_CF0C, 0xE8EC_F641, Some(0x12C), &mut output, &mut unused);
            let (initial_energy, energy_count) = capacity_list(this, 0x70, 0x48EB_D0B1, 0x88EC_F76C, Some(0x130), &mut output, &mut unused);
            let mut treatment = initial_treatment; let mut incinerators = initial_incinerators; let mut energy = initial_energy;
            let (allocator, _) = get_allocator(0);
            let sentinel = vcall!(allocator, 0x0C => fn(u32) -> *mut u8, 12);
            ListNode::from_ptr(sentinel).set_next(sentinel); ListNode::from_ptr(sentinel).set_previous(sentinel);
            let mut list = sentinel;
            vcall!(sim.neighbors(), 0x78 => fn(u32, *mut *mut u8), 0x4773_FFE5, &mut list);
            let mut node = ListNode::from_ptr(list).next(); let mut imports = 0i32;
            while node != list {
                let deal = ListNode::from_ptr(node).occupant();
                let used = vcall!(deal, 0x50 => fn() -> i32); let amount = vcall!(deal, 0x28 => fn() -> i32);
                if greater(F80::from_i32(amount.wrapping_sub(used)), *zero()) {
                    let amount = vcall!(deal, 0x28 => fn() -> i32); let functions = table(deal);
                    let share = rounded(mul_float(F80::from_i32(amount), *tenth())); imports = imports.wrapping_add(share);
                    let used = saved_call(deal, functions, 0x50, &[]);
                    saved_call(deal, functions, 0x54, &[used.wrapping_add(share as u32)]);
                    sim.set_imported(sim.imported().wrapping_add(i64::from(share)));
                }
                node = ListNode::from_ptr(node).next();
            }
            vcall!(sim.neighbors(), 0x78 => fn(u32, *mut *mut u8), 0x4773_FFE4, &mut list);
            let mut node = ListNode::from_ptr(list).next();
            while node != list {
                let deal = ListNode::from_ptr(node).occupant();
                let used = vcall!(deal, 0x50 => fn() -> i32); let amount = vcall!(deal, 0x28 => fn() -> i32);
                let room = amount.wrapping_sub(used);
                if room > 0 {
                    let amount = vcall!(deal, 0x28 => fn() -> i32);
                    let share = rounded(mul_float(F80::from_i32(amount), *tenth())).min(room);
                    let functions = table(deal); let used = saved_call(deal, functions, 0x50, &[]);
                    saved_call(deal, functions, 0x54, &[used.wrapping_add(share as u32)]);
                    sim.set_export_capacity(sim.export_capacity().wrapping_add(share));
                }
                node = ListNode::from_ptr(node).next();
            }
            let offered = F80::from_i32(imports).to_f32();
            let left = sub_from_float(dispose(this, offered, &mut incinerators, &mut energy), offered).ftol() as i32;
            vcall!(this, 0x74 => fn(i32), left);
            let mut factor_storage = MaybeUninit::<Factors>::uninit();
            factors(sim, factor_storage.as_mut_ptr());
            let factors = factor_storage.assume_init_ref();
            let mut visits = 0u32;
            while visits < sim.visits() {
                visit(this, sim, factors, &mut treatment, &mut incinerators, &mut energy);
                visits = visits.wrapping_add(1);
            }
            let room = common.capacity().wrapping_sub(common.supply());
            if room < 0 {
                let excess = room.wrapping_neg(); let offered = F80::from_i32(excess).to_f32();
                let mut consumed = dispose(this, offered, &mut incinerators, &mut energy).ftol() as i32;
                let remaining = excess.wrapping_sub(consumed) as u32;
                if remaining > *valve_maximum() { consumed = consumed.wrapping_add(remaining.wrapping_sub(*valve_maximum()) as i32); }
                vcall!(this, 0x74 => fn(i32), consumed.wrapping_neg());
            }
            for (offset, capacity, used, total, remaining, count) in [
                (0x68, 0xC8EB_D131, 0x28EC_F510, initial_treatment, treatment, treatment_count),
                (0x6C, 0xA8EB_CF0C, 0xE8EC_F641, initial_incinerators, incinerators, incinerator_count),
                (0x70, 0x48EB_D0B1, 0x88EC_F76C, initial_energy, energy, energy_count),
            ] {
                let mut amount = total.wrapping_sub(remaining);
                if amount != 0 { distribute(this, this.add(offset), capacity, used, total, &mut amount, count); }
            }
            maintenance(this, this.add(0x68)); maintenance(this, this.add(0x6C)); maintenance(this, this.add(0x74));
            let result = clear_deals(&mut list); let sentinel = list;
            let (allocator, edx) = get_allocator((result >> 32) as u32);
            virtual_call(allocator, 0x10, table(allocator), edx, &[sentinel as u32])
        }
    }
}

/// This file's replacements.
pub const PORTS: &[Port] = &[fc4p_006e7fd0::PORT];

#[cfg(test)]
mod tests;
