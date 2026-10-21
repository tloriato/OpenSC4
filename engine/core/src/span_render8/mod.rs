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

//! Span renderer for 8-bit targets (`nGZDrawSoftRender::cSpanRender8`).
//!
//! Written only from `docs/specs/span-render-8.md`; `§n` references point at
//! that spec. One byte per pixel. The renderer never treats that byte as a
//! palette index or a colour: the solid entries store a byte the caller has
//! already mapped (the mapping is the target buffer's, SR8-1, so here it is an
//! explicit input), and the blit and gradient entries store values derived
//! only from source alpha and the draw colour's alpha byte: an **alpha
//! (coverage) plane**.
//!
//! Five render entry points (§2-§6) and the 14 x 12 dispatch table of §7 with
//! its 125 distinct bodies ([`DISPATCH`]).
//!
//! Bounded API: the original works on raw pointers with no clipping. Here the
//! target is a byte slice with an origin offset and a signed stride, and every
//! address a call would touch is checked **before anything is written**: a
//! call renders every span or returns an error and leaves the target
//! untouched. Inputs on which the original crashes or overruns (count 0,
//! unchecked table indices, a zero tile size, reads outside the source; §1,
//! §6, §7.3, §7.5, §9 last point) are errors here; that is a choice of this
//! API, not behaviour of the original.
//!
//! Which shipped path creates an 8-bit render target (SR8-2) is open, so no
//! glue consumes this module yet.

mod blt;
#[cfg(test)]
mod tests;

pub use blt::{
    blt_kind, body_reads_texel, column_value, render_blt, render_blt_aliased, texel_alpha, BltParams, Column, Geometry,
    Slot, SourceImage, BODY_FILL, BODY_TILED_FILL, DISPATCH, KEY_MASKS,
};

/// Errors of the bounded API. None of these exist in the original.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanError {
    /// Empty span list: the original's `do … while (--n)` walks 2^32 records
    /// for `n = 0` (§1); not modelled.
    EmptySpanList,
    /// A destination pixel of span `span` lies outside the target slice.
    DestOutOfBounds { span: usize },
    /// A texel read by span `span` lies outside the source slice (stretch
    /// is not bounds-checked by the original, §7.5).
    SourceOutOfBounds { span: usize },
    /// `row = key byte + 2*f` >= 14 or `kind` >= 12: unchecked in the
    /// original (§6).
    BadKernelIndex { row: u32, kind: u32 },
    /// Tiled slot with source width 0, or height 0 in a body that reads a
    /// texel: the original's unsigned `div` faults (§7.3, §7.5).
    ZeroTileSize,
}

/// One span record (§1): two packed points `(y << 16) | x`. Pixel count is
/// `end - start` on the whole dwords (wrapping); exclusive end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Span of `width` pixels from `(x, y)`, packed as the original packs it.
    pub fn new(x: u16, y: u16, width: u16) -> Span {
        let start = ((y as u32) << 16) | x as u32;
        Span {
            start,
            end: start.wrapping_add(width as u32),
        }
    }
    pub fn x(self) -> u32 {
        self.start & 0xFFFF
    }
    pub fn y(self) -> u32 {
        self.start >> 16
    }
    /// `end - start` as a 32-bit difference (§1). A record with `end` before
    /// `start` gives a huge count, which the bounded API rejects.
    pub fn count(self) -> u32 {
        self.end.wrapping_sub(self.start)
    }
}

/// 8-bit render target (§1 destination record, `+0` and `+4` only): pixel
/// (x, y) at `origin + y*stride + x` bytes into `data`.
pub struct Target<'a> {
    pub data: &'a mut [u8],
    /// Byte offset of pixel (0, 0) in `data` (the record's base pointer).
    pub origin: usize,
    /// Row stride in bytes; may be negative (§1).
    pub stride: i32,
}

/// Byte offset of `origin + y*stride + bpp*x` if `len` bytes from it fit in
/// `size`. Address maths in i128 so nothing wraps.
pub(crate) fn offset(origin: usize, stride: i32, x: u64, y: u64, bpp: u64, len: u64, size: usize) -> Option<usize> {
    let off = origin as i128 + y as i128 * stride as i128 + (bpp as i128) * (x as i128);
    if off < 0 || off + len as i128 > size as i128 {
        None
    } else {
        Some(off as usize)
    }
}

impl Target<'_> {
    /// Checks every span (count bytes, linear from its start pixel) fits;
    /// returns the start offsets.
    pub(crate) fn check_spans(&self, spans: &[Span]) -> Result<Vec<usize>, SpanError> {
        if spans.is_empty() {
            return Err(SpanError::EmptySpanList);
        }
        spans
            .iter()
            .enumerate()
            .map(|(i, s)| {
                offset(
                    self.origin,
                    self.stride,
                    s.x() as u64,
                    s.y() as u64,
                    1,
                    s.count() as u64,
                    self.data.len(),
                )
                .ok_or(SpanError::DestOutOfBounds { span: i })
            })
            .collect()
    }
}

/// RenderSolid (§2, slot 1): every span pixel receives the **low byte** of
/// `colour` verbatim; bits 8-31 are ignored. `colour` is the context's solid
/// colour, already mapped by the target buffer (SR8-1: what that byte means
/// for an 8-bit buffer is open, so the caller supplies it).
pub fn render_solid(spans: &[Span], target: &mut Target, colour: u32) -> Result<(), SpanError> {
    let offs = target.check_spans(spans)?;
    let byte = colour as u8;
    for (s, off) in spans.iter().zip(offs) {
        target.data[off..off + s.count() as usize].fill(byte);
    }
    Ok(())
}

/// RenderAlpha (§3, slot 3): ignores `argb` entirely and calls RenderSolid
/// with `src` (a virtual call through this vtable). No blending happens.
pub fn render_alpha(spans: &[Span], target: &mut Target, src: u32, argb: u32) -> Result<(), SpanError> {
    let _ = argb; // §3: never read.
    render_solid(spans, target, src)
}

/// Gradient record (§4, 0x30 bytes at `ctx+0x44`): start values, per-x and
/// per-y steps for four channels, 16.16-style fixed point. Only channel 3
/// is read by this renderer.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gradient {
    pub start: [u32; 4],
    pub dx: [u32; 4],
    pub dy: [u32; 4],
}

impl Gradient {
    /// The stored byte at (x, y): `(c3 + dx3*x + dy3*y) >> 16 & 0xFF`,
    /// 32-bit wrapping (§4).
    pub fn value_at(&self, x: u32, y: u32) -> u8 {
        let v = self.start[3]
            .wrapping_add(self.dy[3].wrapping_mul(y))
            .wrapping_add(self.dx[3].wrapping_mul(x));
        (v >> 16) as u8
    }
}

/// RenderGradient (§4, slot 2): per span `V = c3 + dy3*y + dx3*start.x`,
/// store bits 16-23 of `V`, add `dx3` per pixel. Channels 0-2 are ignored.
/// (The original also overwrites its own fourth stack argument with `dx3`;
/// that has no observable effect and is not modelled.)
pub fn render_gradient(spans: &[Span], target: &mut Target, g: &Gradient) -> Result<(), SpanError> {
    let offs = target.check_spans(spans)?;
    for (s, off) in spans.iter().zip(offs) {
        let mut v = g.start[3]
            .wrapping_add(g.dy[3].wrapping_mul(s.y()))
            .wrapping_add(g.dx[3].wrapping_mul(s.x()));
        for px in &mut target.data[off..off + s.count() as usize] {
            *px = (v >> 16) as u8;
            v = v.wrapping_add(g.dx[3]);
        }
    }
    Ok(())
}

/// RenderAlphaGradient (§5, slot 4): a tail jump to RenderGradient through
/// this vtable. No blending.
pub fn render_alpha_gradient(spans: &[Span], target: &mut Target, g: &Gradient) -> Result<(), SpanError> {
    render_gradient(spans, target, g)
}
