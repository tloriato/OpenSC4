// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The 16-bit R5G6B5 span renderer (`nGZDrawSoftRender::cSpanRender565`,
//! `docs/specs/span-render-565.md`): the five methods of the class
//! ([`render_solid`], [`render_gradient`], [`render_alpha`],
//! [`render_alpha_gradient`], [`render_blt`]), the shared blend rule
//! ([`blend`]), and the 168-slot blitter dispatch table ([`blit`]).
//!
//! **Memory model.** The original works on raw pointers with no clipping
//! and no bounds check (§1). The core works on one byte arena (`mem`) and
//! the records carry signed byte offsets into it instead of pointers, so a
//! source may alias the destination exactly as in the original (§1, order
//! and overlap). Every pixel and texel address is computed in 64-bit
//! arithmetic and checked against the arena; an address outside it is
//! [`SpanError::OutOfBounds`]. This is a *precondition* of the core, not a
//! claim about the original, which would read or write wherever its 32-bit
//! wrapping pointer arithmetic lands. Pixels already written before such an
//! error stay written (spans are processed in order, pixels ascending).
//!
//! Likewise an empty span list ([`SpanError::EmptySpanList`]; the original
//! loops `do … while (--n)` and walks 2³² records), a span that is not on
//! one row with `end.x ≥ start.x` ([`SpanError::InvalidSpan`]; the
//! original takes `end − start` as a 32-bit pixel count), a dispatch index
//! outside the 14 × 12 table ([`SpanError::SlotOutsideTable`]; the original
//! reads outside its table) and a zero tiling size
//! ([`SpanError::TiledZeroSize`]; the original faults in `div`) are
//! rejected as preconditions. None of these rejections reproduces or
//! verifies the original's behaviour for such inputs (spec §9).

pub mod blend;
pub mod blit;

#[cfg(test)]
mod tests;

pub use blend::{alpha_weight, blend, ALPHA_WEIGHT_MUL};
pub use blit::{
    body_address, slot, texel_to_565, Addressing, BltOp, Slot, SourceFormat, SLOT_BODY, TABLE_COLUMNS, TABLE_ROWS,
};

/// Why a call was rejected (preconditions of the core, see the module docs).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpanError {
    /// `n = 0`: the original wraps and walks 2³² records (§1).
    EmptySpanList,
    /// Span `index` is not on one row with `end.x ≥ start.x` (§1).
    InvalidSpan { index: usize },
    /// A pixel or texel address outside the arena.
    OutOfBounds { addr: i64 },
    /// `row = key_flag + 2·format` or `kind` outside the 14 × 12 table (§6).
    SlotOutsideTable { row: u32, kind: u32 },
    /// Tiled slot with source width or height 0 (§6.1: the original faults).
    TiledZeroSize,
}

/// One span record (§1): two packed points, x in the low 16 bits, y in the
/// high 16 bits, unsigned. Pixels `start.x … end.x − 1` of row `start.y`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// The span `x0 … x1 − 1` on row `y`.
    pub fn new(x0: u16, x1: u16, y: u16) -> Self {
        Span {
            start: (y as u32) << 16 | x0 as u32,
            end: (y as u32) << 16 | x1 as u32,
        }
    }

    /// `(x, y, count)` of a valid span, `None` otherwise.
    fn row(self) -> Option<(u32, u32, u32)> {
        let (x0, y0) = (self.start & 0xFFFF, self.start >> 16);
        let (x1, y1) = (self.end & 0xFFFF, self.end >> 16);
        (y0 == y1 && x1 >= x0).then(|| (x0, y0, x1 - x0))
    }
}

/// Destination record (§1). Only `+0` (here a byte offset into the arena)
/// and `+4` (row stride in bytes, may be negative) are read by this class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dest {
    pub base: i64,
    pub stride: i32,
}

/// Source record of RenderBlt (§6): texel (0, 0) as a byte offset into the
/// arena, stride in bytes, width W and height H (read by the tiled slots).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Source {
    pub base: i64,
    pub stride: i32,
    pub width: u32,
    pub height: u32,
}

/// The 32-byte `b` record of RenderBlt (§6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BltParams {
    /// `+0`, `+4`: stretch start (16.16) or tiled origin.
    pub u0: u32,
    pub v0: u32,
    /// `+8`, `+0xC`: stretch steps (16.16).
    pub du: u32,
    pub dv: u32,
    /// `+0x10`: source-format index `f` (§6.1).
    pub format: u32,
    /// `+0x14`: ARGB draw colour.
    pub argb: u32,
    /// `+0x18`: colour key (compared against the masked texel, §7.3).
    pub key: u32,
    /// `+0x1C`: key flag byte (added to the row index unchecked).
    pub key_flag: u8,
}

/// The 0x30-byte gradient record (§4.1): start values, per-x and per-y
/// steps; channel 0 is B, 1 G, 2 R, 3 the alpha weight.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Gradient {
    pub c: [u32; 4],
    pub dx: [u32; 4],
    pub dy: [u32; 4],
}

impl Gradient {
    /// `V_i = c_i + dx_i·x + dy_i·y` (32-bit wrapping).
    pub fn value(&self, i: usize, x: u32, y: u32) -> u32 {
        self.c[i]
            .wrapping_add(self.dx[i].wrapping_mul(x))
            .wrapping_add(self.dy[i].wrapping_mul(y))
    }
}

pub(crate) struct Mem<'a>(pub &'a mut [u8]);

impl Mem<'_> {
    fn range(&self, addr: i64, len: usize) -> Result<usize, SpanError> {
        let a = usize::try_from(addr).map_err(|_| SpanError::OutOfBounds { addr })?;
        match a.checked_add(len) {
            Some(end) if end <= self.0.len() => Ok(a),
            _ => Err(SpanError::OutOfBounds { addr }),
        }
    }
    pub fn read(&self, addr: i64, len: usize) -> Result<u32, SpanError> {
        let a = self.range(addr, len)?;
        Ok(self.0[a..a + len].iter().rev().fold(0u32, |v, &b| v << 8 | b as u32))
    }
    pub fn read16(&self, addr: i64) -> Result<u16, SpanError> {
        self.read(addr, 2).map(|v| v as u16)
    }
    pub fn write16(&mut self, addr: i64, v: u16) -> Result<(), SpanError> {
        let a = self.range(addr, 2)?;
        self.0[a..a + 2].copy_from_slice(&v.to_le_bytes());
        Ok(())
    }
}

/// Pixel address `base + y × stride + 2 × x` (§1), unwrapped.
pub(crate) fn pixel_addr(dst: &Dest, x: u32, y: u32) -> i64 {
    dst.base + y as i64 * dst.stride as i64 + 2 * x as i64
}

/// Validates the list and yields `(index, x, y, count)` per span.
pub(crate) fn rows(spans: &[Span]) -> Result<Vec<(u32, u32, u32)>, SpanError> {
    if spans.is_empty() {
        return Err(SpanError::EmptySpanList);
    }
    spans
        .iter()
        .enumerate()
        .map(|(i, s)| s.row().ok_or(SpanError::InvalidSpan { index: i }))
        .collect()
}

/// RenderSolid (slot 1, §2): the low 16 bits of `colour`, verbatim, into
/// every pixel of every span.
pub fn render_solid(mem: &mut [u8], spans: &[Span], dst: &Dest, colour: u32) -> Result<(), SpanError> {
    let mut m = Mem(mem);
    for (x, y, count) in rows(spans)? {
        for k in 0..count {
            m.write16(pixel_addr(dst, x + k, y), colour as u16)?;
        }
    }
    Ok(())
}

/// RenderAlpha (slot 3, §3): `B(src & 0xFFFF, D, w)` with `w` from bits
/// 24–31 of `argb` ([`alpha_weight`]).
pub fn render_alpha(mem: &mut [u8], spans: &[Span], dst: &Dest, src: u32, argb: u32) -> Result<(), SpanError> {
    let w = alpha_weight((argb >> 24) as u8);
    let mut m = Mem(mem);
    for (x, y, count) in rows(spans)? {
        for k in 0..count {
            let a = pixel_addr(dst, x + k, y);
            let d = m.read16(a)?;
            m.write16(a, blend(src as u16, d, w))?;
        }
    }
    Ok(())
}

/// The three colour accumulators of §4.2, set at `(x, y)`.
struct GradAcc {
    b: u32,
    g: u32,
    r: u32,
    sb: u32,
    sg: u32,
    sr: u32,
}

impl GradAcc {
    fn new(g: &Gradient, x: u32, y: u32) -> Self {
        GradAcc {
            // B: start shifted right logically, step arithmetically (§4.2):
            // the step's low 3 bits are dropped, so B drifts.
            b: g.value(0, x, y) >> 3,
            sb: ((g.dx[0] as i32) >> 3) as u32,
            g: g.value(1, x, y) << 3,
            sg: g.dx[1] << 3,
            r: g.value(2, x, y) << 8,
            sr: g.dx[2] << 8,
        }
    }
    fn pixel(&self) -> u16 {
        ((self.r >> 27) << 11 | ((self.g >> 21) & 0x3F) << 5 | ((self.b >> 16) & 0x1F)) as u16
    }
    fn step(&mut self) {
        self.b = self.b.wrapping_add(self.sb);
        self.g = self.g.wrapping_add(self.sg);
        self.r = self.r.wrapping_add(self.sr);
    }
}

/// RenderGradient (slot 2, §4.2), including the B-channel drift.
pub fn render_gradient(mem: &mut [u8], spans: &[Span], dst: &Dest, g: &Gradient) -> Result<(), SpanError> {
    let mut m = Mem(mem);
    for (x, y, count) in rows(spans)? {
        let mut acc = GradAcc::new(g, x, y);
        for k in 0..count {
            m.write16(pixel_addr(dst, x + k, y), acc.pixel())?;
            acc.step();
        }
    }
    Ok(())
}

/// RenderAlphaGradient (slot 4, §5): colour as §4.2, weight
/// `w = (V3 × 0x84) >> 26` (0…63; above 32 the packed blend rule applies).
pub fn render_alpha_gradient(mem: &mut [u8], spans: &[Span], dst: &Dest, g: &Gradient) -> Result<(), SpanError> {
    let mut m = Mem(mem);
    for (x, y, count) in rows(spans)? {
        let mut acc = GradAcc::new(g, x, y);
        let mut a = g.value(3, x, y).wrapping_mul(0x84);
        let sa = g.dx[3].wrapping_mul(0x84);
        for k in 0..count {
            let p = pixel_addr(dst, x + k, y);
            let d = m.read16(p)?;
            m.write16(p, blend(acc.pixel(), d, a >> 26))?;
            acc.step();
            a = a.wrapping_add(sa);
        }
    }
    Ok(())
}

/// RenderBlt (slot 5, §6): dispatches to `rows[key_flag + 2·format][kind]`
/// (§7) and runs that body on the span list.
pub fn render_blt(
    mem: &mut [u8],
    spans: &[Span],
    dst: &Dest,
    src: &Source,
    b: &BltParams,
    kind: u32,
) -> Result<(), SpanError> {
    let row = (b.key_flag as u32).wrapping_add(b.format.wrapping_mul(2));
    let s = slot(row, kind).ok_or(SpanError::SlotOutsideTable { row, kind })?;
    blit::run(&mut Mem(mem), s, spans, dst, src, b)
}
