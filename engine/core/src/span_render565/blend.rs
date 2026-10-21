// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The 565 blend rule `B(S, D, w)` (§3, helper `0x00845450`) and the
//! RenderAlpha weight.

/// Multiplier of the RenderAlpha weight (§3 step 1).
pub const ALPHA_WEIGHT_MUL: u32 = 0x0081_8180;

/// RenderAlpha weight `w = (a8 × 0x818180) >> 26`, `0…32` (§3 step 1).
pub fn alpha_weight(a8: u8) -> u32 {
    (a8 as u32).wrapping_mul(ALPHA_WEIGHT_MUL) >> 26
}

/// `B(S, D, w)` (§3 step 2), all arithmetic 32-bit wrapping. For `w ≤ 32`
/// this is the per-channel `floor((w·S + (32 − w)·D + 16) / 32)`; for
/// larger `w` (only RenderAlphaGradient with an out-of-range alpha, §5) the
/// packed formula with its inter-channel borrows is the rule.
pub fn blend(s: u16, d: u16, w: u32) -> u16 {
    let (s, d) = (s as u32, d as u32);
    let g = (s & 0x7E0)
        .wrapping_sub(d & 0x7E0)
        .wrapping_mul(w)
        .wrapping_add(((d & 0x7E0) + 0x10) * 32)
        & 0xFC00;
    let rb = (s & 0xF81F)
        .wrapping_sub(d & 0xF81F)
        .wrapping_mul(w)
        .wrapping_add((d & 0xF81F) * 32 + 0x8010)
        & 0x1F_03E0;
    (g.wrapping_add(rb) >> 5) as u16
}
