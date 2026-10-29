// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Trip-source completion statistics and average-length publication.

use super::*;

layout! {
    /// Completed trip records are embedded in a linked list.
    pub struct CompletedTrip {
        0x00 => next / set_next: *mut u8,
        0x20 => count / set_count: u32,
        0x24 => kind / set_kind: u8,
        0x26 => flags / set_flags: u8,
        0x27 => status / set_status: u8,
        0x2C => length / set_length: f32,
        0x30 => travel_types / set_travel_types: u32,
    }
}
layout! {
    /// The completion pass's statistics, grids and tuning.
    pub struct SourceTotalsSim {
        0x44 => length_grid / set_length_grid: *mut u8,
        0x430 => mode_counts / set_mode_counts: *mut u8,
        0x93C => pass_total / set_pass_total: u32,
    }
}
game_data! { static unsigned_bias: f32 = 0x00A8_0AA8; }
game_data! { static byte_scale: f32 = 0x00A9_D8B0; }
game_data! { static rounding_bias: f32 = 0x00AB_32D8; }
game_data! { static network_manager: *mut u8 = 0x00B4_3D8C; }
game_fn! { fn set_float_property(holder: *mut u8, id: u32, value: f32) = "C" 0x005F_E4F0; }

unsafe fn unsigned(value: u32) -> F80 {
    let value80 = F80::from_i32(value as i32);
    if value as i32 >= 0 { value80 } else { add_float(value80, unsafe { read_unaligned(unsigned_bias()) }) }
}
unsafe fn increment(p: *mut u8, offset: usize, value: u32) {
    unsafe { store(p, offset, word(p, offset).wrapping_add(value)); }
}

port! {
    /// Marks each trip's last-pass outcome, accumulates commute/freight and mode totals, then publishes
    /// the weighted length over the source rectangle or to its first matching boundary connection.
    /// Probes contribute to the source mean; failed freight probes also contribute to freight totals.
    #[port(address = 0x0070_F0A0, owner = "traffic", name = "cSC4TrafficSimulator::CompleteTripSourceCalculations")]
    pub unsafe extern "thiscall" fn fc4p_0070f0a0(this: *mut u8, source: *mut u8) {
        unsafe {
            let sim = TrafficSim::from_ptr(this);
            let totals = SourceTotalsSim::from_ptr(this);
            let list = vcall!(source, 0x0C => fn() -> *mut u8);
            let mut node = word(word(list, 0) as *mut u8, 0) as *mut u8;
            let list = vcall!(source, 0x0C => fn() -> *mut u8);
            let end = word(list, 0) as *mut u8;
            if node == end { return; }
            let mut sum = F80::from_f32(0.0);
            let mut count = 0u32;
            let mut failed = false;
            loop {
                let trip = CompletedTrip::from_ptr(node);
                totals.set_pass_total(totals.pass_total().wrapping_add(1));
                match trip.status() {
                    3 => {
                        trip.set_flags(trip.flags() | 8);
                        let length = multiply_float(unsigned(trip.count()), trip.length());
                        let stored_length = length.to_f32();
                        count = count.wrapping_add(trip.count());
                        sum = sum + length;
                        if trip.flags() & 2 == 0 {
                            let kind = trip.kind();
                            if kind < 3 {
                                increment(this, 0x1CC + usize::from(kind) * 4, trip.count());
                                increment(this, 0x1D8 + usize::from(trip.kind()) * 4, trip.count());
                                let length = multiply_float(unsigned(trip.count()), trip.length());
                                sim.set_commute_sum(add_float(length, sim.commute_sum()).to_f32());
                                sim.set_commute_count(sim.commute_count().wrapping_add(trip.count()));
                            } else if kind == 3 {
                                sim.set_freight_sum(add_float(F80::from_f32(stored_length), sim.freight_sum()).to_f32());
                                sim.set_freight_count(sim.freight_count().wrapping_add(trip.count()));
                            }
                            for mode in 0..9usize {
                                if trip.travel_types() & (1 << mode) != 0 { increment(totals.mode_counts(), mode * 4, trip.count()); }
                            }
                        }
                    }
                    5 => {
                        trip.set_flags(trip.flags() & !8);
                        failed = true;
                        let length = multiply_float(unsigned(trip.count()), sim.commute_maximum());
                        count = count.wrapping_add(trip.count());
                        sum = sum + length;
                        let kind = trip.kind();
                        if kind < 3 && trip.flags() & 2 == 0 {
                            increment(this, 0x1CC + usize::from(kind) * 4, trip.count());
                        }
                        if trip.kind() == 3 {
                            let length = multiply_float(unsigned(trip.count()), sim.commute_maximum());
                            sim.set_freight_sum(add_float(length, sim.freight_sum()).to_f32());
                            sim.set_freight_count(sim.freight_count().wrapping_add(trip.count()));
                        }
                    }
                    _ => trip.set_flags(trip.flags() & !8),
                }
                node = trip.next();
                if node == end { break; }
            }
            if count == 0 { return; }
            let average = (sum / F80::from_i32(count as i32)).to_f32();
            let mut rect = [0i32; 4];
            vcall!(source, 0x14 => fn(*mut i32), rect.as_mut_ptr());
            let (x0, z0) = (rect[0], rect[1]);
            if x0 >= sim.min_cell_x() && z0 >= sim.min_cell_z() && x0 <= sim.max_cell_x() && z0 <= sim.max_cell_z() {
                let value = if failed { 0 } else {
                    let scaled = multiply_float(divide_float(F80::from_f32(average), sim.commute_maximum()), read_unaligned(byte_scale()));
                    let encoded = add_float(scaled, read_unaligned(rounding_bias())).to_f32().to_bits().wrapping_add(0xB4C0_0000);
                    if encoded > 255 { 255 } else if encoded == 0 { 1 } else { encoded as u8 }
                };
                let vector = vcall!(totals.length_grid(), 0x60 => fn() -> *mut u8);
                let mut x = x0;
                while x <= rect[2] {
                    let mut z = z0;
                    while z <= rect[3] {
                        let rows = word(vector, 0) as *mut u8;
                        let row = word(rows, x.wrapping_mul(12) as usize) as *mut u8;
                        row.wrapping_offset(z as isize).write(value);
                        z = z.wrapping_add(1);
                    }
                    x = x.wrapping_add(1);
                }
            } else {
                for kind in 0..13u32 {
                    let connection = vcall!(read_unaligned(network_manager()), 0x2C => fn(u32, i32, i32) -> *mut u8, kind, x0, z0);
                    if !connection.is_null() {
                        let holder = vcall!(connection, 0x0C => fn() -> *mut u8);
                        set_float_property(holder, 0xCA94_7077, average);
                        break;
                    }
                }
            }
        }
    }
}

pub const PORTS: &[Port] = &[fc4p_0070f0a0::PORT];

#[cfg(test)]
mod tests;
