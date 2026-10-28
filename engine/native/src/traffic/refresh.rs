// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors

//! Incremental rebuilding of destination availability.

use super::*;
use super::network::{register_port, registers};
use crate::abi::{Conv, RawCall};

layout! {
    /// The scan cursor uses inclusive bounds and advances x before z.
    pub struct Scan {
        0x00 => state / set_state: u8,
        0x04 => x / set_x: i32,
        0x08 => z / set_z: i32,
        0x0C => step / set_step: i32,
        0x10 => minimum_x / set_minimum_x: i32,
        0x18 => maximum_x / set_maximum_x: i32,
        0x1C => maximum_z / set_maximum_z: i32,
    }
}
layout! {
    /// Refresh fields relative to the traffic simulator's primary interface.
    pub struct RefreshSim {
        0x4C => destination_rows / set_destination_rows: *mut u8,
        0x1B0 => free_counts / set_free_counts: [u32; 4],
        0x818 => cursor / set_cursor: *mut u8,
    }
}
game_fn! { #[allow(dead_code)] fn budget_expired(this: *mut u32) -> u32 = "thiscall" 0x0068_BF80; }
game_data! { static pacing: [u32; 2] = 0x00B4_B318; }
game_data! { static initialized: u32 = 0x00B4_B320; }
game_data! { static lot_manager: *mut u8 = 0x00B4_3D08; }

register_port!(0x0070_F6E0, fc4p_0070f6e0, "cSC4TrafficSimulator::WorkOnRefreshCommuteMap", refresh, 4, [8]);
unsafe extern "fastcall" fn refresh(this: *mut u8, out: *mut Registers, budget: *mut u32) {
    unsafe {
        if initialized().cast::<u8>().read() & 1 == 0 {
            write_unaligned(initialized(), read_unaligned(initialized()) | 1);
            write_unaligned(pacing(), [1024, 0]);
        }
        let mut scope = [read_unaligned(budget), read_unaligned(budget.add(1)), 0, word(pacing().cast(), 0), 0, pacing() as u32];
        let manager = read_unaligned(lot_manager());
        let sim = RefreshSim::from_ptr(this);
        let mut work = 0u32;
        let (mut eax, completed) = loop {
            work = work.wrapping_add(scope[3]);
            scope[2] = work;
            if work & 1023 == 0 {
                let result = registers(RawCall::new(Conv::Thiscall, crate::abi::target(0x0068_BF80), scope.as_mut_ptr() as u32, &[]));
                if result.eax as u8 != 0 { break (result.eax, false); }
            }
            let scan = Scan::from_ptr(sim.cursor());
            if scan.state() == 2 { break (scan.as_ptr() as u32, true); }
            if scan.state() == 0 { scan.set_state(1); }
            else {
                let x = scan.x().wrapping_add(scan.step());
                scan.set_x(x);
                if x > scan.maximum_x() {
                    let z = scan.z().wrapping_add(scan.step());
                    scan.set_x(scan.minimum_x());
                    scan.set_z(z);
                    if z > scan.maximum_z() { scan.set_state(2); break (scan.as_ptr() as u32, true); }
                }
            }
            let (x, z) = (scan.x(), scan.z());
            let row = word(sim.destination_rows(), x.wrapping_add(1).wrapping_mul(12) as usize) as *mut u8;
            let destination = row.wrapping_add(z.wrapping_add(1) as usize);
            destination.write(destination.read() & 0xF8);
            let primary = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 1);
            if !primary.is_null() {
                let holder = vcall!(primary, 0x0C => fn() -> *mut u8);
                for category in 0u32..4 {
                    if vcall!(this, 0x50 => fn(*mut u8, u32) -> u32, holder, category) != 0 {
                        destination.write(destination.read() | 1 << category);
                        let counter = this.add(0x1B0 + category as usize * 4);
                        store(counter, 0, word(counter, 0).wrapping_add(1));
                    }
                }
            } else {
                let lot = vcall!(manager, 0x18 => fn(i32, i32, u32) -> *mut u8, x, z, 0);
                if !lot.is_null() {
                    let holder = vcall!(lot, 0x0C => fn() -> *mut u8);
                    for category in 0u32..4 {
                        if vcall!(this, 0x50 => fn(*mut u8, u32) -> u32, holder, category) != 0 {
                            destination.write(destination.read() | 1 << category);
                        }
                    }
                }
            }
            // The budget helper may replace the local work accumulator.
            work = scope[2];
        };
        let count = work >> 10;
        let mut ecx = 0;
        if count != 0 {
            if count as i32 > 20 {
                eax = word(pacing().cast(), 4).wrapping_add(1);
                store(pacing().cast(), 4, eax);
                if eax as i32 >= 16 {
                    eax = word(pacing().cast(), 0);
                    store(pacing().cast(), 4, 0);
                    if eax > 1 { eax >>= 1; store(pacing().cast(), 0, eax); }
                }
            } else if (count as i32) < 5 {
                eax = word(pacing().cast(), 4).wrapping_sub(1);
                store(pacing().cast(), 4, eax);
                if eax as i32 <= -4 {
                    eax = word(pacing().cast(), 0);
                    store(pacing().cast(), 4, 0);
                    if eax < 1024 {
                        if completed { eax = eax.wrapping_mul(2); store(pacing().cast(), 0, eax); }
                        else { ecx = eax.wrapping_mul(2); store(pacing().cast(), 0, ecx); }
                    }
                }
            }
        }
        out.write(Registers { eax: (eax & !255) | u32::from(completed), ecx, edx: 0 });
    }
}

pub const PORTS: &[Port] = &[fc4p_0070f6e0::PORT];

#[cfg(test)]
mod tests;
