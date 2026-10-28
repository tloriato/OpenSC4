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

//! Small functions that several classes share (template instances, folded identical bodies, utilities), in
//! groups by address so lanes work in separate files.

use crate::port::Port;

pub mod g01;
pub mod g02;
pub mod g03;
pub mod g04;
pub mod g05;
pub mod g06;
pub mod g07;
pub mod g08;
pub mod g09;
pub mod g10;
pub mod g11;
pub mod g12;

const COUNT: usize = g01::PORTS.len()
    + g02::PORTS.len()
    + g03::PORTS.len()
    + g04::PORTS.len()
    + g05::PORTS.len()
    + g06::PORTS.len()
    + g07::PORTS.len()
    + g08::PORTS.len()
    + g09::PORTS.len()
    + g10::PORTS.len()
    + g11::PORTS.len()
    + g12::PORTS.len();
const ALL: [Port; COUNT] = crate::port::concat(&[g01::PORTS, g02::PORTS, g03::PORTS, g04::PORTS, g05::PORTS, g06::PORTS, g07::PORTS, g08::PORTS, g09::PORTS, g10::PORTS, g11::PORTS, g12::PORTS]);

/// This module's replacements.
pub const PORTS: &[Port] = &ALL;
