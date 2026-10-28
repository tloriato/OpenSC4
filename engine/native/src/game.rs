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
    /// Releases an allocation through the game's heap.
    pub fn heap_free(allocation: *mut u8) = "C" 0x005E_5620;
}

game_fn! {
    /// `cSC4ResponseCurve::GetOutputValue(x)`: the curve's output at `x` (foundations §4), returned in ST0.
    pub fn response_curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250;
}

game_fn! {
    /// Destroys the land value simulator's owned containers.
    pub fn land_value_destroy(this: *mut u8) = "thiscall" 0x006B_DD10;
}

game_fn! {
    /// Allocates storage for a standard message through the game's allocator.
    pub fn message_allocate(size: u32) -> *mut u8 = "C" 0x0091_33DA;
}

game_fn! {
    /// Constructs a standard message and returns its primary interface.
    pub fn message_construct(this: *mut u8) -> *mut u8 = "thiscall" 0x0091_34D6;
}
