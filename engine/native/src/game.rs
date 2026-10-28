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
    /// Copies the nested byte vectors.
    pub fn grid_bytes_assign(this: *mut u8, source: *mut u8) -> u32 = "thiscall" 0x0051_17D0;
}

game_fn! {
    /// Advances a developer map cursor.
    pub fn developer_cursor_next(cursor: *mut *mut u8) -> u32 = "thiscall" 0x005D_17C0;
}

game_fn! {
    /// Adds the distance from water effect.
    #[allow(clippy::too_many_arguments)]
    pub fn land_value_water_effect(target: *mut u8, count_x: i32, count_z: i32, mask: *mut u8, source: u32, radius: f32, near: f32, far: f32) = "C" 0x005E_5390;
}

game_fn! {
    /// `cSC4ResponseCurve::GetOutputValue(x)`: the curve's output at `x` (foundations §4), returned in ST0.
    pub fn response_curve_output(curve: *mut u8, x: f32) -> St0 = "thiscall" 0x005F_4250;
}

game_fn! {
    /// Resets a scanning iterator.
    pub fn scan_reset(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x0060_C860;
}

game_fn! {
    /// Yields the next scan coordinate in AL.
    pub fn scan_next(this: *mut u8, x: *mut i32, z: *mut i32) -> u32 = "thiscall" 0x0060_C8D0;
}

game_fn! {
    /// Updates the shared pacing stride.
    pub fn idle_budget_finish(this: *mut u8) -> u32 = "thiscall" 0x0068_B960;
}

game_fn! {
    /// Tests the idle deadline.
    pub fn idle_budget_expired(this: *mut u8) -> u32 = "thiscall" 0x0068_BF80;
}

game_fn! {
    /// Starts a deadline with shared adaptive pacing.
    pub fn idle_budget_new(this: *mut u8, milliseconds: u32, shared: *mut u32) -> u32 = "thiscall" 0x0068_CC20;
}

game_fn! {
    /// Tests the tract for zoning.
    pub fn land_value_tract_is_zoned(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x006B_D540;
}

game_fn! {
    /// Tests the tract for land.
    pub fn land_value_tract_has_land(this: *mut u8, x: i32, z: i32) -> u32 = "thiscall" 0x006B_D5C0;
}

game_fn! {
    /// Notifies consumers of the completed pass.
    pub fn land_value_end_cycle(this: *mut u8) = "thiscall" 0x006B_D6B0;
}

game_fn! {
    /// Sets the altitude curve parameters.
    pub fn land_value_setup_altitude(this: *mut u8) = "thiscall" 0x006B_D750;
}

game_fn! {
    /// Looks up the altitude contribution.
    pub fn land_value_altitude_effect(this: *mut u8, altitude: f32) -> u32 = "thiscall" 0x006B_D810;
}

game_fn! {
    /// Combines developer and working grids through the callback.
    pub fn land_value_process_developer(target: *mut u8, source: *mut u8, callback: usize, context: *mut u8) = "C" 0x006B_DA30;
}

game_fn! {
    /// Constructs the idle profiling timer.
    pub fn idle_timer_new(this: *mut u8, kind: u32) -> u32 = "thiscall" 0x0088_FEDF;
}

game_fn! {
    /// Starts the idle profiling timer.
    pub fn idle_timer_start(this: *mut u8) = "thiscall" 0x0089_05C4;
}

game_data! {
    /// City services.
    pub static idle_city: *mut u8 = 0x00B4_3D68;
}

game_data! {
    /// History service.
    pub static idle_history: *mut u8 = 0x00B4_3DC0;
}

game_data! {
    /// Average scan pacing.
    pub static idle_average_pacing: [u32; 2] = 0x00B4_90E4;
}

game_data! {
    /// Altitude scan pacing.
    pub static idle_altitude_pacing: [u32; 2] = 0x00B4_90EC;
}

game_data! {
    /// Slope scan pacing.
    pub static idle_slope_pacing: [u32; 2] = 0x00B4_90F4;
}

game_data! {
    /// Statistics scan pacing.
    pub static idle_statistics_pacing: [u32; 2] = 0x00B4_90FC;
}

game_data! {
    /// Base scan pacing.
    pub static idle_base_pacing: [u32; 2] = 0x00B4_9104;
}

game_data! {
    /// Initialized pacing groups.
    pub static idle_pacing_flags: u32 = 0x00B4_910C;
}
