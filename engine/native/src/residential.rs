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

//! `cSC4ResidentialSimulator`, converted one function at a time. Its game functions and globals are declared in this file.

use crate::port::Port;

layout! {
    /// Residential simulator fields, relative to its primary interface.
    pub struct ResidentialSim {
        0x10 => references / set_references: i32,
        0x14 => message_server / set_message_server: *mut u8,
        0x38 => proximity_low / set_proximity_low: *mut u8,
        0x3C => proximity_medium / set_proximity_medium: *mut u8,
        0x40 => proximity_high / set_proximity_high: *mut u8,
        0x48 => population_low / set_population_low: *mut u8,
        0x4C => population_medium / set_population_medium: *mut u8,
        0x50 => population_high / set_population_high: *mut u8,
        0x7C => eq_low / set_eq_low: *mut u8,
        0x80 => eq_medium / set_eq_medium: *mut u8,
        0x84 => eq_high / set_eq_high: *mut u8,
        0x88 => hq_low / set_hq_low: *mut u8,
        0x8C => hq_medium / set_hq_medium: *mut u8,
        0x90 => hq_high / set_hq_high: *mut u8,
        0xA4 => population / set_population: i32,
        0xA8 => running_population / set_running_population: i32,
        0x118 => health_strike / set_health_strike: u8,
        0x178 => school_strike / set_school_strike: u8,
        0x1C4 => age_count / set_age_count: [i32; 20],
        0x214 => age_eq_sum / set_age_eq_sum: [i32; 20],
        0x324 => coverage_slop / set_coverage_slop: f32,
        0x348 => life_sum_low / set_life_sum_low: u32,
        0x34C => life_sum_high / set_life_sum_high: u32,
    }
}

pub mod civic;
pub mod coverage;
pub mod grid;
pub mod interface;
pub mod population;
pub mod proximity;
pub mod query;
pub mod quotient;
pub mod save;
pub mod served;
pub mod service;
pub mod strike;

const COUNT: usize = interface::PORTS.len()
    + proximity::PORTS.len()
    + quotient::PORTS.len()
    + service::PORTS.len()
    + civic::PORTS.len()
    + strike::PORTS.len()
    + query::PORTS.len()
    + population::PORTS.len()
    + save::PORTS.len()
    + grid::PORTS.len()
    + served::PORTS.len()
    + coverage::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[
    interface::PORTS,
    proximity::PORTS,
    quotient::PORTS,
    service::PORTS,
    civic::PORTS,
    strike::PORTS,
    query::PORTS,
    population::PORTS,
    save::PORTS,
    grid::PORTS,
    served::PORTS,
    coverage::PORTS,
]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;
