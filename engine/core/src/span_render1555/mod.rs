//! Span renderer for 16-bit 1555 buffers (`nGZDrawSoftRender::cSpanRender1555`),
//! clean-room from `docs/specs/span-render-1555.md` (cited as §n below).
//!
//! One class serves colour types 4 (`R5G5B5`) and 6 (`A1R5G5B5`). Destination
//! word, little-endian `u16`: bits 0-4 B, 5-9 G, 10-14 R, bit 15 A.
//!
//! Memory model. The original works on raw 32-bit pointers and does no
//! clipping or bounds checks (§1). Here a surface is a byte slice plus the
//! byte offset of pixel (0, 0) (`base`, may be negative or past the end, as a
//! biased pointer can be) and a signed byte stride; every address is
//! `base + (32-bit wrapping offset)`. Preconditions the original leaves to the
//! caller are explicit [`Error`]s here instead of memory faults:
//!
//! - an empty span list (the original's `do … while (--n)` walks 2^32 records);
//! - a span whose end is not on the start's row or lies left of it (§1);
//! - a destination pixel outside the buffer (checked for every span before
//!   anything is written);
//! - a source texel outside the buffer (checked as it is read: pixels already
//!   written stay written; the original reads whatever memory is there);
//! - a dispatch row ≥ 14 or column ≥ 12 (§6: no range check in the original);
//! - an empty tile (`TW = 0` or `TH = 0`, a divide fault in the original, §7.6).

use crate::x87::{ftol32, r24};

#[cfg(test)]
mod tests;

/// One span record (§1): packed points, x in the low 16 bits, y in the high 16.
/// Pixels `start.x … end.x − 1` of row `start.y` are written, end-exclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

impl Span {
    /// The span covering `count` pixels of row `y` from column `x`.
    pub fn row(x: u16, y: u16, count: u16) -> Span {
        let start = (u32::from(y) << 16) | u32::from(x);
        Span {
            start,
            end: start + u32::from(count),
        }
    }
}

/// Destination surface (§1: `+0` base pointer of pixel (0, 0), `+4` stride in bytes).
#[derive(Debug)]
pub struct Surface<'a> {
    pub buf: &'a mut [u8],
    /// Byte offset in `buf` of pixel (0, 0).
    pub base: i64,
    pub stride: i32,
}

/// The 24-byte source record of RenderBlt (§6.1).
#[derive(Clone, Copy, Debug)]
pub struct BltSource<'a> {
    pub buf: &'a [u8],
    /// Byte offset in `buf` of the record's pixel pointer (`+0`), after the
    /// caller's bias (§6.1 plain mode, §7.6 tiled mode).
    pub base: i64,
    /// `+4` row stride in bytes.
    pub stride: i32,
    /// `+8` width; the tile width `TW` in the tiled columns (§7.6).
    pub width: u32,
    /// `+0xC` height; the tile height `TH` in the tiled columns.
    pub height: u32,
    /// `+0x10` colour type (no body reads it).
    pub color_type: u32,
    /// `+0x14` bytes per texel (no body reads it).
    pub bpp: u32,
}

/// The 32-byte blit record of RenderBlt (§6, §6.1).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BlitRecord {
    /// `+0x00` u0 (stretch, tiled).
    pub u0: u32,
    /// `+0x04` v0 (stretch, tiled).
    pub v0: u32,
    /// `+0x08` du (stretch).
    pub du: u32,
    /// `+0x0C` dv (stretch).
    pub dv: u32,
    /// `+0x10` source-format index.
    pub format: u32,
    /// `+0x14` the ARGB draw colour.
    pub color: u32,
    /// `+0x18` the colour key (already masked by the caller).
    pub key: u32,
    /// `+0x1C` the colour-key flag byte.
    pub key_flag: u8,
}

/// The 0x30-byte gradient record (§4.1): `V_i(x, y) = c_i + dx_i·x + dy_i·y`,
/// channel 0 → B, 1 → G, 2 → R, 3 → alpha weight.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct GradientRecord {
    pub c: [i32; 4],
    pub dx: [i32; 4],
    pub dy: [i32; 4],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// `n = 0` (§1: the original underflows the count).
    EmptySpanList,
    /// Span `index` crosses rows or ends left of its start (§1).
    InvalidSpan { index: usize },
    /// Span `index` touches a destination byte outside the buffer.
    DestOutOfBounds { index: usize },
    /// A source texel lies outside the source buffer.
    SourceOutOfBounds,
    /// `b.key_flag + 2 × b.format` ≥ 14 (§6).
    RowOutOfRange { row: u64 },
    /// `kind` ≥ 12 (§6).
    KindOutOfRange { kind: u32 },
    /// Tiled column with `TW = 0` or `TH = 0` (§7.6).
    EmptyTile,
}

/// A checked span: pixel count, first x, row y, byte offset of the first pixel.
struct Run {
    x0: u32,
    y: u32,
    count: u32,
    off: usize,
}

/// Validates the span list and the destination range of every span (§1).
fn runs(spans: &[Span], dst: &Surface) -> Result<Vec<Run>, Error> {
    if spans.is_empty() {
        // Compat (§1): the original walks 2^32 records; callers must never pass 0.
        return Err(Error::EmptySpanList);
    }
    let mut out = Vec::with_capacity(spans.len());
    for (index, s) in spans.iter().enumerate() {
        let (x0, y) = (s.start & 0xFFFF, s.start >> 16);
        let (x1, y1) = (s.end & 0xFFFF, s.end >> 16);
        if y1 != y || x1 < x0 {
            return Err(Error::InvalidSpan { index });
        }
        let count = x1 - x0;
        let rel = (y as i32).wrapping_mul(dst.stride).wrapping_add((2 * x0) as i32);
        let start = dst.base + i64::from(rel);
        let end = start + 2 * i64::from(count);
        if count > 0 && (start < 0 || end > dst.buf.len() as i64) {
            return Err(Error::DestOutOfBounds { index });
        }
        out.push(Run {
            x0,
            y,
            count,
            off: start.max(0) as usize,
        });
    }
    Ok(out)
}

fn get16(buf: &[u8], off: usize) -> u16 {
    u16::from_le_bytes([buf[off], buf[off + 1]])
}

fn put16(buf: &mut [u8], off: usize, v: u16) {
    buf[off..off + 2].copy_from_slice(&v.to_le_bytes());
}

/// The packed blend of §5 (`0x008313D9`-`0x00831421`) with weight `w`, also the
/// §3 / §7.4 blend: for `w ≤ 32` it equals
/// `C = floor(((32 − w)·D + w·S + 16) / 32)` per channel. Bit 15 of the result
/// is always 1; bit 15 of `s` and `d` is ignored.
pub fn blend(s: u16, d: u16, w: u32) -> u16 {
    let (s, d) = (u32::from(s), u32::from(d));
    let (sp, dp) = (s & 0x7C1F, d & 0x7C1F);
    let (sg, dg) = (s & 0x03E0, d & 0x03E0);
    let rb = sp
        .wrapping_sub(dp)
        .wrapping_mul(w)
        .wrapping_add(dp << 5)
        .wrapping_add(0x4010)
        >> 5;
    let g = sg.wrapping_sub(dg).wrapping_mul(w).wrapping_add((dg + 0x10) << 5) >> 5;
    ((rb | 0x83E0) & (g | 0xFC1F)) as u16
}

/// RenderAlpha's weight (§3 step 1): `(a8 × 0x818180) >> 26`, 0…32.
pub fn alpha_weight(argb: u32) -> u32 {
    (argb >> 24).wrapping_mul(0x0081_8180) >> 26
}

/// RenderSolid (§2, slot 1): the low 16 bits of `colour` verbatim to every pixel.
pub fn render_solid(spans: &[Span], dst: &mut Surface, colour: u32) -> Result<(), Error> {
    let v = colour as u16;
    for r in runs(spans, dst)? {
        for k in 0..r.count as usize {
            put16(dst.buf, r.off + 2 * k, v);
        }
    }
    Ok(())
}

/// RenderAlpha (§3, slot 3): blends the 1555 colour `src` (bit 15 ignored)
/// with weight [`alpha_weight`]`(argb)`; output bit 15 is always 1.
pub fn render_alpha(spans: &[Span], dst: &mut Surface, src: u32, argb: u32) -> Result<(), Error> {
    let w = alpha_weight(argb);
    let s = src as u16;
    for r in runs(spans, dst)? {
        for k in 0..r.count as usize {
            let o = r.off + 2 * k;
            let d = get16(dst.buf, o);
            put16(dst.buf, o, blend(s, d, w));
        }
    }
    Ok(())
}

/// `V_i(x0, y)` of §4.1 in 32-bit wrapping arithmetic.
fn v_at(g: &GradientRecord, i: usize, x: u32, y: u32) -> u32 {
    (g.c[i] as u32)
        .wrapping_add((g.dx[i] as u32).wrapping_mul(x))
        .wrapping_add((g.dy[i] as u32).wrapping_mul(y))
}

/// RenderGradient (§4.2, slot 2). Reproduces the 1555 quirk bit for bit: the
/// start values are not shifted, only the G and R steps (×32, ×1024); bit 15 = 0.
pub fn render_gradient(spans: &[Span], dst: &mut Surface, g: &GradientRecord) -> Result<(), Error> {
    for r in runs(spans, dst)? {
        let mut a0 = v_at(g, 0, r.x0, r.y);
        let mut a1 = v_at(g, 1, r.x0, r.y);
        // The original adds 0x80000000 here; it never reaches bits 26-30.
        let mut a2 = v_at(g, 2, r.x0, r.y).wrapping_add(0x8000_0000);
        let s0 = g.dx[0] as u32;
        let s1 = (g.dx[1] as u32) << 5;
        let s2 = (g.dx[2] as u32) << 10;
        for k in 0..r.count as usize {
            let b = (a0 >> 16) & 31;
            let gg = (a1 >> 21) & 31;
            let rr = (a2 >> 26) & 31;
            put16(dst.buf, r.off + 2 * k, ((rr << 10) | (gg << 5) | b) as u16);
            a0 = a0.wrapping_add(s0);
            a1 = a1.wrapping_add(s1);
            a2 = a2.wrapping_add(s2);
        }
    }
    Ok(())
}

/// RenderAlphaGradient (§5, slot 4). The weight `w = Aa >> 26` is 0…63; above
/// 32 the packed blend of [`blend`] still applies (§5 *[compat]*).
pub fn render_alpha_gradient(spans: &[Span], dst: &mut Surface, g: &GradientRecord) -> Result<(), Error> {
    for r in runs(spans, dst)? {
        // B: start shifted logically, step arithmetically, each truncated (§5).
        let mut ab = v_at(g, 0, r.x0, r.y) >> 3;
        let sb = (g.dx[0] >> 3) as u32;
        let mut ag = v_at(g, 1, r.x0, r.y) << 2;
        let sg = (g.dx[1] as u32) << 2;
        let mut ar = v_at(g, 2, r.x0, r.y) << 7;
        let sr = (g.dx[2] as u32) << 7;
        let mut aa = v_at(g, 3, r.x0, r.y).wrapping_mul(0x84);
        let sa = (g.dx[3] as u32).wrapping_mul(0x84);
        for k in 0..r.count as usize {
            let s = (((ar >> 26) & 31) << 10) | (((ag >> 21) & 31) << 5) | ((ab >> 16) & 31);
            let o = r.off + 2 * k;
            let d = get16(dst.buf, o);
            put16(dst.buf, o, blend(s as u16, d, aa >> 26));
            ab = ab.wrapping_add(sb);
            ag = ag.wrapping_add(sg);
            ar = ar.wrapping_add(sr);
            aa = aa.wrapping_add(sa);
        }
    }
    Ok(())
}

/// Bytes per texel of a dispatch row's source (§7.7).
pub fn texel_bytes(format: u32) -> u32 {
    match format {
        0..=3 => 2,
        4 | 5 => 4,
        _ => 1,
    }
}

/// The key-test mask of a keyed row's format (§7.3); 0 for the unreachable
/// 8-bit row 13 (it compares 0 with the key).
pub fn key_mask(format: u32) -> u32 {
    match format {
        0 => 0x0FFF,
        1 | 2 => 0x7FFF,
        3 => 0xFFFF,
        4 | 5 => 0x00FF_FFFF,
        _ => 0,
    }
}

/// Column 0 conversion of a raw texel to 1555 (§7.3).
// A4444 channel positions: the spec names the format `A4444` (colour type 7)
// and puts A in bits 12-15 (key mask 0x0FFF, "A = bit 15"); R in bits 8-11,
// G 4-7, B 0-3 is the literal A-R-G-B reading by analogy with A8R8G8B8 (SR1555-5).
pub fn convert(format: u32, s: u32) -> u16 {
    let v = match format {
        0 => {
            let c = |sh: u32| {
                let c4 = (s >> sh) & 15;
                (c4 << 1) | (c4 >> 3)
            };
            (s & 0x8000) | (c(8) << 10) | (c(4) << 5) | c(0)
        }
        1 => s & 0xFFFF,
        2 => (s & 0x7FFF) | 0x8000,
        3 => 0x8000 | (((s >> 11) & 31) << 10) | (((s >> 6) & 31) << 5) | (s & 31),
        4 | 5 => {
            let a = if format == 4 { (s >> 31) << 15 } else { 0x8000 };
            a | (((s >> 19) & 31) << 10) | (((s >> 11) & 31) << 5) | ((s >> 3) & 31)
        }
        _ => 0x7FFF | ((s & 0x80) << 8),
    };
    v as u16
}

/// Expansion to 8-bit `[A, R, G, B]` for Modulate / ModulateAlpha (§7.4).
pub fn expand(format: u32, s: u32) -> [u32; 4] {
    let c5 = |sh: u32| {
        let c = (s >> sh) & 31;
        (c << 3) | (c >> 2)
    };
    match format {
        0 => [12, 8, 4, 0].map(|sh| ((s >> sh) & 15) * 17),
        1 | 2 => {
            let a = if format == 2 || s & 0x8000 != 0 { 0xFF } else { 0 };
            [a, c5(10), c5(5), c5(0)]
        }
        3 => {
            let g6 = (s >> 5) & 63;
            [0xFF, c5(11), (g6 << 2) | (g6 >> 4), c5(0)]
        }
        4 | 5 => {
            let a = if format == 4 { s >> 24 } else { 0xFF };
            [a, (s >> 16) & 0xFF, (s >> 8) & 0xFF, s & 0xFF]
        }
        _ => [s & 0xFF, 0xFF, 0xFF, 0xFF],
    }
}

/// Modulate (§7.4): `c' = (c·m + 128) >> 8` per channel, `m` the matching byte
/// of the ARGB draw colour.
pub fn modulate(c: [u32; 4], color: u32) -> [u32; 4] {
    let m = [color >> 24, (color >> 16) & 0xFF, (color >> 8) & 0xFF, color & 0xFF];
    [0, 1, 2, 3].map(|i| (c[i] * m[i] + 128) >> 8)
}

/// Packing by the A8888 rule of §7.3 (truncation, A = alpha ≥ 128).
pub fn pack8888(c: [u32; 4]) -> u16 {
    (((c[0] >> 7) << 15) | ((c[1] >> 3) << 10) | ((c[2] >> 3) << 5) | (c[3] >> 3)) as u16
}

/// One slot's colour operation `col % 4` (§7.7) on texel `s` over `d`.
fn apply(op: u32, format: u32, s: u32, d: u16, color: u32) -> u16 {
    match op {
        0 => convert(format, s),
        1 => blend(convert(format, s), d, color >> 27),
        2 => pack8888(modulate(expand(format, s), color)),
        _ => {
            let m = modulate(expand(format, s), color);
            blend(pack8888(m), d, m[0] >> 3)
        }
    }
}

fn read_texel(src: &BltSource, rel: u32, bytes: u32) -> Result<u32, Error> {
    // 32-bit pointer arithmetic: the offset wraps as a signed 32-bit value.
    let at = src.base + i64::from(rel as i32);
    if at < 0 || at + i64::from(bytes) > src.buf.len() as i64 {
        return Err(Error::SourceOutOfBounds);
    }
    let o = at as usize;
    Ok(match bytes {
        1 => u32::from(src.buf[o]),
        2 => u32::from(get16(src.buf, o)),
        _ => u32::from_le_bytes([src.buf[o], src.buf[o + 1], src.buf[o + 2], src.buf[o + 3]]),
    })
}

/// RenderBlt (§6, slot 5): dispatches to `rows[b.key_flag + 2·b.format][kind]`
/// and runs the slot rule of §7.7. Format and key come from the row index
/// (row / 2, row % 2), as the table lookup does, not from the fields alone.
pub fn render_blt(spans: &[Span], dst: &mut Surface, src: &BltSource, b: &BlitRecord, kind: u32) -> Result<(), Error> {
    let row = u64::from(b.key_flag) + 2 * u64::from(b.format);
    if row >= 14 {
        return Err(Error::RowOutOfRange { row });
    }
    if kind >= 12 {
        return Err(Error::KindOutOfRange { kind });
    }
    let (format, keyed) = ((row / 2) as u32, row % 2 == 1);
    let (mode, op) = (kind / 4, kind % 4);
    if mode == 2 && (src.width == 0 || src.height == 0) {
        // Compat (§7.6): a divide fault in the original.
        return Err(Error::EmptyTile);
    }
    let bytes = texel_bytes(format);
    let mask = key_mask(format);
    let stride = src.stride as u32;
    for r in runs(spans, dst)? {
        // Per-span addressing state (§6.1 plain, §7.5 stretch, §7.6 tiled).
        let row_off = match mode {
            0 => r.y.wrapping_mul(stride),
            1 => (b.v0.wrapping_add(b.dv.wrapping_mul(r.y)) >> 16).wrapping_mul(stride),
            _ => (r.y.wrapping_add(b.v0) % src.height).wrapping_mul(stride),
        };
        let mut u = match mode {
            0 => r.x0,
            1 => b.u0.wrapping_add(b.du.wrapping_mul(r.x0)),
            _ => r.x0.wrapping_add(b.u0) % src.width,
        };
        for k in 0..r.count as usize {
            let col = if mode == 1 { u >> 16 } else { u };
            let s = read_texel(src, row_off.wrapping_add(col.wrapping_mul(bytes)), bytes)?;
            u = match mode {
                0 => u.wrapping_add(1),
                1 => u.wrapping_add(b.du),
                _ if u + 1 == src.width => 0,
                _ => u + 1,
            };
            if keyed && (s & mask) == b.key {
                continue;
            }
            let o = r.off + 2 * k;
            let d = get16(dst.buf, o);
            put16(dst.buf, o, apply(op, format, s, d, b.color));
        }
    }
    Ok(())
}

// ---- Caller-side values (§4.1, §6.1, §7.5, §7.6), pure helpers ----

/// `fistp` with round-to-nearest-even (the FPU default, §4.1); out-of-range
/// and NaN give the integer indefinite `0x80000000`.
fn fistp32(v: f64) -> i32 {
    // Round half to even by hand (f64::round_ties_even is above the MSRV).
    let f = v.floor();
    let r = if v - f > 0.5 || (v - f == 0.5 && f % 2.0 != 0.0) {
        f + 1.0
    } else {
        f
    };
    if r.is_nan() || !(-2_147_483_648.0..=2_147_483_647.0).contains(&r) {
        i32::MIN
    } else {
        r as i32
    }
}

/// §4.1 `R(v)`: the product rounded to float, then `fistp`.
fn r_fixed(p: f32, k: f64) -> i32 {
    fistp32(r24(f64::from(p) * k))
}

/// `cGZDrawContextRenderToBuffer::SetGradientTransform` (§4.1): `None` for a
/// null `p` (the gradient flag is cleared, nothing else changes).
// The c_i expression is evaluated with one 24-bit rounding per x87 operation,
// the game's precision control per foundations §18; §4.1 itself does not
// state the precision (SR1555-5).
pub fn gradient_from_transform(p: Option<&[f32; 12]>, x0: f32, y0: f32) -> Option<GradientRecord> {
    const K: f64 = 16_711_680.0;
    let p = p?;
    let mut g = GradientRecord::default();
    for i in 0..4 {
        g.dx[i] = r_fixed(p[4 + i], K);
        g.dy[i] = r_fixed(p[8 + i], K);
        let s = r_fixed(p[i], K).wrapping_add(0x8000);
        let a = r24(f64::from(g.dx[i]) * f64::from(x0));
        let b = r24(f64::from(g.dy[i]) * f64::from(y0));
        let sum = r24(a + b);
        g.c[i] = ftol32(r24(f64::from(s) - sum));
    }
    Some(g)
}

/// §6.1: source-format index of a texture colour type; `None` = nothing drawn.
pub fn src_format_index(color_type: u32) -> Option<u32> {
    match color_type {
        7 => Some(0),
        6 => Some(1),
        4 => Some(2),
        5 => Some(3),
        9 => Some(4),
        8 => Some(5),
        2 => Some(6),
        _ => None,
    }
}

/// §6.1: the key flag and key the caller stores for a format index. Index 6
/// forces the flag to 0 (row 13 unreachable); its key is left as given.
pub fn caller_key(format: u32, flag: bool, key: u32) -> (u8, u32) {
    match format {
        0..=5 => (u8::from(flag), key & key_mask(format)),
        _ => (0, key),
    }
}

/// §6.1: `kind = alpha + 2·modulate`, `+8` if tiling, else `+4` if stretching.
pub fn blt_kind(alpha: bool, modulate: bool, tiling: bool, stretch: bool) -> u32 {
    let base = u32::from(alpha) + 2 * u32::from(modulate);
    if tiling {
        base + 8
    } else if stretch {
        base + 4
    } else {
        base
    }
}

/// §7.6 caller: `(u0, v0)` for tile size `TW × TH` and origin `(ox, oy)`, all
/// unsigned 32-bit; `None` for an empty tile (a divide fault in the original).
pub fn tile_origin(ox: u32, oy: u32, tw: u32, th: u32) -> Option<(u32, u32)> {
    const M: u32 = 0x7FFF_FFFF;
    if tw == 0 || th == 0 {
        return None;
    }
    let u0 = (M - M % tw).wrapping_sub(ox) % tw;
    let v0 = (M - M % th).wrapping_sub(oy) % th;
    Some((u0, v0))
}

/// §7.5 caller: `(u0, v0) = ((du >> 1) − ox, (dv >> 1) − oy)`, arithmetic
/// shifts, 32-bit wrap; `ox`, `oy` already 16.16.
pub fn stretch_origin(du: i32, dv: i32, ox: i32, oy: i32) -> (u32, u32) {
    ((du >> 1).wrapping_sub(ox) as u32, (dv >> 1).wrapping_sub(oy) as u32)
}
