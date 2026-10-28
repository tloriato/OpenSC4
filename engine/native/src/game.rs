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
    /// Construct a string from a zero-terminated name.
    pub fn string_from_cstr(this: *mut u8, name: *const u8) -> u32 = "thiscall" 0x0040_8480;
}
game_fn! {
    /// Allocate game-owned storage.
    pub fn allocate(size: u32) -> *mut u8 = "C" 0x005E_55E0;
}
game_fn! {
    /// Free game-owned storage.
    pub fn free(pointer: *mut u8) -> u32 = "C" 0x005E_5620;
}
game_fn! {
    /// `cSC4ResponseCurve::GetOutputValue(x)`: the curve's output at `x` (foundations §4), returned in ST0.
    pub fn response_curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250;
}
game_fn! {
    /// Set response curve data from a property.
    pub fn response_curve_set(this: *mut u8, property: *mut u8) -> u32 = "thiscall" 0x005F_4620;
}
game_fn! {
    /// Read an unsigned byte property, leaving the destination on absence.
    pub fn property_u8(holder: *mut u8, id: u32, output: *mut u8) -> u32 = "C" 0x005F_D3C0;
}
game_fn! {
    /// Construct a scanning iterator.
    pub fn scanning_iterator_new(this: *mut u8) -> *mut u8 = "thiscall" 0x0060_CE90;
}
game_fn! {
    /// Find or insert a developer weight by interface pointer.
    pub fn developer_weight(this: *mut u8, key: *const *mut u8) -> *mut u32 = "thiscall" 0x006A_3170;
}
game_fn! {
    /// Resize the altitude histogram, filling new elements from the supplied value.
    pub fn histogram_resize(this: *mut u8, count: u32, value: *const u32) -> u32 = "thiscall" 0x006B_DF10;
}
game_fn! {
    /// Read the land value tuning exemplar.
    pub fn land_value_tuning(this: *mut u8) -> u32 = "thiscall" 0x006B_E110;
}
game_fn! {
    /// Advance one land value pipeline state within a time budget.
    pub fn land_value_idle(this: *mut u8, budget: u32) -> u32 = "thiscall" 0x006B_E7B0;
}
game_fn! {
    /// Construct an empty response curve.
    pub fn response_curve_new(this: *mut u8) -> *mut u8 = "thiscall" 0x007D_0410;
}
game_fn! {
    /// Free an array owned by a response curve or string.
    pub fn free_array(pointer: *mut u8) -> u32 = "C" 0x0090_CF63;
}

game_data! { pub static grid_u8_serial_vtable: u8 = 0x00A8_78A4; }
game_data! { pub static grid_u8_vtable: u8 = 0x00A8_78C0; }
game_data! { pub static grid_f32_serial_vtable: u8 = 0x00A8_7C04; }
game_data! { pub static grid_f32_vtable: u8 = 0x00A8_7C20; }
game_data! { pub static land_value_agent_name: u8 = 0x00A8_9864; }
game_data! { pub static resource_manager: *mut u8 = 0x00B4_3CA8; }
game_data! { pub static message_server: *mut u8 = 0x00B4_3CCC; }
game_data! { pub static city: *mut u8 = 0x00B4_3CEC; }
game_data! { pub static terrain: *mut u8 = 0x00B4_3CF4; }
game_data! { pub static zone_manager: *mut u8 = 0x00B4_3D14; }
game_data! { pub static simulator: *mut u8 = 0x00B4_3D5C; }
game_data! { pub static growth_developer: *mut u8 = 0x00B4_3D68; }
game_data! { pub static history: *mut u8 = 0x00B4_3DC0; }
game_data! { pub static land_value_started: u8 = 0x00B4_90E0; }

game_fn! {
    /// Erases a developer-map subtree, visiting right children before the node and its left children.
    pub fn land_value_erase_subtree(map: *mut u8, node: *mut u8) -> u32 = "thiscall" 0x006B_D9F0;
}

game_fn! {
    /// Releases owned storage through the game's delete function; keeps its EAX result.
    pub fn delete_storage(storage: *mut u8) -> u32 = "C" 0x0090_CF63;
}
