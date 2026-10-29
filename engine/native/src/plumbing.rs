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

//! `cSC4PlumbingSimulator`: replacements operating on the game's original objects and interfaces.

use crate::port::Port;

layout! {
    /// Fields used by the plumbing queries and callbacks, from the primary interface.
    pub struct PlumbingSim {
        0x10 => reference_count / set_reference_count: i32,
        0x18 => city / set_city: *mut u8,
        0x34 => watered_grid / set_watered_grid: *mut u8,
        0xBC => producers / set_producers: *mut u8,
        0x128 => exported / set_exported: u32,
        0x174 => pipes / set_pipes: u32,
        0x178 => distressed / set_distressed: u32,
        0x188 => buildings / set_buildings: u32,
        0x18C => watered_buildings / set_watered_buildings: u32,
    }
}

pub mod com;
pub mod deque;
pub mod io;
pub mod occupants;
pub mod queries;

const COUNT: usize =
    com::PORTS.len() + deque::PORTS.len() + io::PORTS.len() + occupants::PORTS.len() + queries::PORTS.len();
const ALL: [Port; COUNT] =
    crate::port::concat(&[com::PORTS, deque::PORTS, io::PORTS, occupants::PORTS, queries::PORTS]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;
