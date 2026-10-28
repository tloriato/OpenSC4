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

//! Game functions that converted code calls directly (not through a vtable), by original address.

game_fn! {
    /// `cSC4ResponseCurve::GetOutputValue(x)`: the curve's output at `x` (foundations §4), returned in ST0.
    pub fn response_curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250;
}

game_fn! {
    /// Erases a developer-map subtree, visiting right children before the node and its left children.
    pub fn land_value_erase_subtree(map: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x006B_D9F0;
}

game_fn! {
    /// Releases owned storage through the game's delete function; keeps its EAX result.
    pub fn delete_storage(storage: *mut u8) -> u32 = "C" 0x0090_CF63;
}
