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

//! `cSC4LandValueSimulator`: the two-grid pass and its callbacks (006bda30 ProcessTwoGridsWithCallback<uint8,int16>, 006bd9f0, 006bdd10).

#[allow(unused_imports)]
use super::*;
use crate::port::Port;

/// This file's replacements.
pub const PORTS: &[Port] = &[];
