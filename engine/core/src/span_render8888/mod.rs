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

//! Span renderer for A8R8G8B8 / X8R8G8B8 targets (`nGZDrawSoftRender::cSpanRender8888`).
//!
//! Written only from `docs/specs/span-render-8888.md`; `§n` references point at
//! that spec. The five entry points (§3-§7) and the 14 x 12 blit kernel table
//! (§8, §9) are pure functions over byte memory.
//!
//! Memory model. The original works on raw pointers into one address space, so
//! a blit source may overlap its destination. Here a call gets one byte slice
//! ([`Mem::Shared`]: source and destination are offsets into the same bytes and
//! may overlap) or two ([`Mem::Split`]: distinct buffers). Pixels are processed
//! in the order the spec gives (§2 spans in list order, §9.1 pixels left to
//! right, each pixel reading its texel, then the destination if the op reads it,
//! then writing), with every read going to the live memory: a texel written by
//! an earlier pixel of the same call is read back with its new value. No
//! snapshot of the source and no memmove-style direction choice is made.
//!
//! Bounded API (choices of this module, not behaviour of the original, which
//! has no checks at all, §2): every address a call would touch is checked
//! before anything is written, so a call either renders every span or returns
//! a [`SpanError`] and leaves memory untouched. Address arithmetic is done
//! exactly (64-bit) instead of 32-bit wrapping; an address outside the slice is
//! an error even where the original's wrapped pointer might land elsewhere.

mod blt;
mod methods;
#[cfg(test)]
mod tests;

pub use blt::{
    keyed_skip, modulate, render_blt, run_kernel, widen, BltDesc, Geometry, Kernel, Op, SourceFormat, SourceImage,
    KERNEL_COUNT, KEY_MASKS,
};
pub use methods::{
    alpha_gradient_weight, blend_alpha_gradient, render_alpha, render_alpha_gradient, render_alpha_weight,
    render_gradient, render_solid, Gradient,
};

/// Errors of the bounded API. None exist in the original (§2, §7, §11).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanError {
    /// Empty span list: the original's decrement-then-test loop would run
    /// 2^32 times for count 0 (§2, U1). Rejected, not modelled.
    EmptySpanList,
    /// A destination pixel of span `span` lies outside the destination bytes.
    DestOutOfBounds { span: usize },
    /// A texel read by span `span` lies outside the source bytes (stretch walks
    /// are not clamped by the original, §9.1, U4).
    SourceOutOfBounds { span: usize },
    /// `src.format > 6`, keyed byte > 1 or `mode > 11`: the original indexes
    /// its table without a check (§7, U2). Rejected (question SR8888-2).
    BadKernelIndex { format: u32, keyed: u8, mode: u32 },
    /// Tiled kernel with source width or height 0: the original's `DIV`
    /// would fault (U4).
    ZeroTileSize,
}

/// Span record (§2): `start`, `end` = `(y << 16) | x`; width = `end - start`
/// on the whole dwords (32-bit wrap; an `end` before `start` gives a huge
/// width, which the bounds check then rejects).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// Span of `width` pixels from (x, y) on one row.
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
    pub fn width(self) -> u32 {
        self.end.wrapping_sub(self.start)
    }
}

/// Destination descriptor (§2 `buffer`, §7 `rectA`): byte offset of pixel
/// (0, 0) within the destination bytes and signed row stride in bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Surface {
    pub base: usize,
    pub stride: i32,
}

/// The bytes a call works on.
pub enum Mem<'a> {
    /// One address space: destination and source offsets index the same
    /// bytes and may overlap.
    Shared(&'a mut [u8]),
    /// Distinct destination and source buffers.
    Split { dst: &'a mut [u8], src: &'a [u8] },
}

impl Mem<'_> {
    pub(crate) fn dst_len(&self) -> usize {
        match self {
            Mem::Shared(m) => m.len(),
            Mem::Split { dst, .. } => dst.len(),
        }
    }
    pub(crate) fn src_len(&self) -> usize {
        match self {
            Mem::Shared(m) => m.len(),
            Mem::Split { src, .. } => src.len(),
        }
    }
    fn src_bytes(&self) -> &[u8] {
        match self {
            Mem::Shared(m) => m,
            Mem::Split { src, .. } => src,
        }
    }
    fn dst_bytes(&mut self) -> &mut [u8] {
        match self {
            Mem::Shared(m) => m,
            Mem::Split { dst, .. } => dst,
        }
    }
    /// Little-endian read of `size` (1, 2 or 4) bytes of the source.
    pub(crate) fn read_src(&self, off: usize, size: usize) -> u32 {
        let b = self.src_bytes();
        (0..size).fold(0u32, |v, i| v | (b[off + i] as u32) << (8 * i))
    }
    pub(crate) fn read_dst(&mut self, off: usize) -> u32 {
        let b = self.dst_bytes();
        u32::from_le_bytes([b[off], b[off + 1], b[off + 2], b[off + 3]])
    }
    pub(crate) fn write_dst(&mut self, off: usize, v: u32) {
        self.dst_bytes()[off..off + 4].copy_from_slice(&v.to_le_bytes());
    }
}

/// Exact byte offset of `base + y*stride + bpp*x`, if `size` bytes there lie
/// within `len`.
pub(crate) fn offset(base: usize, stride: i32, x: i64, y: i64, bpp: i64, len: usize, size: usize) -> Option<usize> {
    let off = base as i64 + y * stride as i64 + bpp * x;
    if off >= 0 && off + size as i64 <= len as i64 {
        Some(off as usize)
    } else {
        None
    }
}

/// Checks every span's destination run (pixels are contiguous along x, so the
/// first and last pixel decide) and returns the offset of each span's first
/// pixel (unused for width 0).
pub(crate) fn check_dest(spans: &[Span], dst: Surface, len: usize) -> Result<Vec<usize>, SpanError> {
    if spans.is_empty() {
        return Err(SpanError::EmptySpanList);
    }
    let mut out = Vec::with_capacity(spans.len());
    for (i, s) in spans.iter().enumerate() {
        let (x, y, w) = (s.x() as i64, s.y() as i64, s.width() as i64);
        if w == 0 {
            out.push(0);
            continue;
        }
        let first = offset(dst.base, dst.stride, x, y, 4, len, 4);
        let last = offset(dst.base, dst.stride, x + w - 1, y, 4, len, 4);
        match (first, last) {
            (Some(f), Some(_)) => out.push(f),
            _ => return Err(SpanError::DestOutOfBounds { span: i }),
        }
    }
    Ok(out)
}

/// The blend rule `B(d, s, w)` of §2, per channel: for R, G, B
/// `out = floor((256*d + w*(s - d) + 128) / 256)`, output alpha `0xFF`, source
/// alpha ignored. Defined for `0 <= w <= 256` (callers in this class never pass
/// more, except RenderAlphaGradient, which uses [`blend_alpha_gradient`]).
pub fn blend(d: u32, s: u32, w: u32) -> u32 {
    debug_assert!(w <= 256);
    let w = w as i32;
    let ch = |sh: u32| {
        let dc = ((d >> sh) & 0xFF) as i32;
        let sc = ((s >> sh) & 0xFF) as i32;
        (((256 * dc + w * (sc - dc) + 128) >> 8) as u32 & 0xFF) << sh
    };
    0xFF00_0000 | ch(16) | ch(8) | ch(0)
}
