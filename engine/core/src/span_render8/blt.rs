// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors. See mod.rs for the full notice.

//! RenderBlt (§6) and the 8-bit dispatch table (§7).

use super::{offset, Span, SpanError, Target};

/// Fill body `F` (§7, §7.3): plain and stretch slots that store `0xFF` and
/// read nothing.
pub const BODY_FILL: u32 = 0x0085_FCB0;
/// Tiled fill body `T` (§7.3): stores `0xFF`, still evaluates
/// `(start.x + u0) mod W` per span.
pub const BODY_TILED_FILL: u32 = 0x0086_1BB0;

const F: u32 = BODY_FILL;
const T: u32 = BODY_TILED_FILL;

/// The 14 x 12 table of §7, row `2*f + key`, column `kind`: body entry
/// addresses, used here as body identities (shared bodies compute the same
/// thing for every slot that uses them, §7).
pub const DISPATCH: [[u32; 12]; 14] = [
    [
        0x85E7F0, F, 0x85E870, 0x85E950, 0x85EA30, F, 0x85EAD0, 0x85EBD0, 0x85ECD0, T, 0x85ED80, 0x85EEA0,
    ],
    [
        0x85EFC0, 0x85F050, 0x85F0D0, 0x85F1D0, 0x85F2D0, 0x85F380, 0x85F410, 0x85F520, 0x85F630, 0x85F6E0, 0x85F780,
        0x85F8B0,
    ],
    [
        0x85F9E0, F, 0x85FA60, 0x85FB40, 0x85FC20, F, 0x85FD00, 0x85FE20, 0x85FF30, T, 0x85FFC0, 0x8600D0,
    ],
    [
        0x8601E0, 0x860260, 0x8602E0, 0x8603E0, 0x8604E0, 0x860580, 0x860610, 0x860730, 0x860850, 0x861510, 0x860900,
        0x860A30,
    ],
    [
        F, F, 0x860B50, 0x860C20, F, F, 0x860CF0, 0x860E00, T, T, 0x860F00, 0x861010,
    ],
    [
        0x860260, 0x860260, 0x861110, 0x861200, 0x860580, 0x860580, 0x8612F0, 0x861400, 0x861510, 0x861510, 0x8615B0,
        0x8616E0,
    ],
    [
        F, F, 0x8617F0, 0x8618D0, F, F, 0x8619B0, 0x861AB0, T, T, 0x861C30, 0x861D50,
    ],
    [
        0x861E70, 0x861E70, 0x861EE0, 0x861FE0, 0x8620E0, 0x8620E0, 0x862170, 0x862280, 0x862390, 0x862390, 0x862420,
        0x862560,
    ],
    [
        0x862690, F, 0x862700, 0x8627B0, 0x862850, F, 0x8628E0, 0x8629A0, 0x862A60, T, 0x862AF0, 0x862BD0,
    ],
    [
        0x862C90, 0x8637B0, 0x862D10, 0x862DC0, 0x862E70, 0x863990, 0x862F10, 0x862FE0, 0x8630B0, 0x863BC0, 0x863150,
        0x863230,
    ],
    [
        F, F, 0x863310, 0x8633C0, F, F, 0x863470, 0x863540, T, T, 0x863610, 0x8636E0,
    ],
    [
        0x8637B0, 0x8637B0, 0x863830, 0x8638E0, 0x863990, 0x863990, 0x863A20, 0x863AF0, 0x863BC0, 0x863BC0, 0x863C50,
        0x863D40,
    ],
    [
        0x863E10, F, 0x863E70, 0x863F20, 0x863FD0, F, 0x864050, 0x864120, 0x8641F0, T, 0x864270, 0x864350,
    ],
    [
        0x864420, 0x864480, 0x8644D0, 0x864580, 0x864630, 0x8646C0, 0x864710, 0x8647E0, 0x8648B0, 0x864940, 0x8649B0,
        0x864A90,
    ],
];

/// Key masks per source format `f` (§7.4). Index 6 (8-bit) is unused: row 13
/// compares no texel bits.
pub const KEY_MASKS: [u32; 7] = [0x0FFF, 0x7FFF, 0x7FFF, 0xFFFF, 0x00FF_FFFF, 0x00FF_FFFF, 0xFF];

/// Column rule (§7.3), `kind & 3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Column {
    Blt,
    ConstantAlpha,
    Modulate,
    ModulateAlpha,
}

/// Addressing (§7.5), `kind >> 2`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geometry {
    Plain,
    Stretch,
    Tiled,
}

/// One slot of [`DISPATCH`]: row `2*f + key` (< 14), column `kind` (< 12).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub row: u32,
    pub kind: u32,
}

impl Slot {
    pub fn new(row: u32, kind: u32) -> Result<Slot, SpanError> {
        if row >= 14 || kind >= 12 {
            return Err(SpanError::BadKernelIndex { row, kind });
        }
        Ok(Slot { row, kind })
    }
    /// Source format of the row (§7.2): 0 A4444 .. 6 8-bit.
    pub fn format(self) -> u32 {
        self.row / 2
    }
    pub fn keyed(self) -> bool {
        self.row & 1 == 1
    }
    pub fn column(self) -> Column {
        match self.kind & 3 {
            0 => Column::Blt,
            1 => Column::ConstantAlpha,
            2 => Column::Modulate,
            _ => Column::ModulateAlpha,
        }
    }
    pub fn geometry(self) -> Geometry {
        match self.kind >> 2 {
            0 => Geometry::Plain,
            1 => Geometry::Stretch,
            _ => Geometry::Tiled,
        }
    }
    pub fn body(self) -> u32 {
        DISPATCH[self.row as usize][self.kind as usize]
    }
    /// Texel size in bytes (§7.5): 2 for f <= 3, 4 for f = 4, 5, 1 for f = 6.
    pub fn bpp(self) -> u32 {
        match self.format() {
            0..=3 => 2,
            4 | 5 => 4,
            _ => 1,
        }
    }
}

/// Whether the slot's body reads a texel (§7.2, §7.3, §7.5). The fill bodies
/// read nothing. Row 13 `ConstantAlpha` compares no texel bits and stores a
/// constant; the spec says its tiled form divides by W only, so it is taken
/// as reading no texel in all three geometries (literal reading).
pub fn body_reads_texel(slot: Slot) -> bool {
    let b = slot.body();
    !(b == BODY_FILL || b == BODY_TILED_FILL || (slot.row == 13 && slot.column() == Column::ConstantAlpha))
}

/// Texel -> alpha `A` (§7.2), the only texel property any body uses.
pub fn texel_alpha(format: u32, texel: u32) -> u8 {
    match format {
        0 => {
            let a = ((texel >> 12) & 0xF) as u8;
            (a << 4) | a
        }
        1 => {
            if texel & 0x8000 != 0 {
                0xFF
            } else {
                0
            }
        }
        4 => (texel >> 24) as u8,
        6 => texel as u8,
        _ => 0xFF,
    }
}

/// Column value (§7.3) for alpha `a` and draw-colour alpha byte `ca`.
/// Modulate is `(a*ca + 128) >> 8` (never above 254: opaque x 0xFF gives
/// 0xFE). ModulateAlpha computes that and discards it: 0xFF.
pub fn column_value(column: Column, a: u8, ca: u8) -> u8 {
    match column {
        Column::Blt => a,
        Column::ConstantAlpha | Column::ModulateAlpha => 0xFF,
        Column::Modulate => ((a as u32 * ca as u32 + 128) >> 8) as u8,
    }
}

/// `kind` as the shared caller forms it (§6): `alpha + 2*modulate`, then
/// `| 8` tiled or `| 4` stretch (tiling wins).
pub fn blt_kind(alpha: bool, modulate: bool, stretch: bool, tiled: bool) -> u32 {
    let k = alpha as u32 + 2 * modulate as u32;
    if tiled {
        k | 8
    } else if stretch {
        k | 4
    } else {
        k
    }
}

/// The `src` record of §6 (`+0` .. `+0xC`): pixel (0, 0) at `origin` bytes
/// into the source buffer, row stride, width W and height H (W and H are
/// used only by the tiled slots).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceImage {
    pub origin: usize,
    pub stride: i32,
    pub width: u32,
    pub height: u32,
}

/// The `b` record of §6 (32 bytes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BltParams {
    pub u0: u32,
    pub v0: u32,
    pub du: u32,
    pub dv: u32,
    /// `+0x10` source-format index `f` (dword; only indexes the table).
    pub format: u32,
    /// `+0x14` ARGB draw colour; only byte 3 (`ca`) is used.
    pub colour: u32,
    /// `+0x18` colour key, compared as a full dword (§7.4).
    pub key: u32,
    /// `+0x1C` key flag, read as a byte and **added** to `2*f` (§6): a value
    /// other than 0/1 selects another row; kept as in the original.
    pub key_flag: u8,
}

impl BltParams {
    /// `rows[key byte + 2*f]` (§6), unchecked in the original.
    pub fn row(&self) -> u32 {
        (self.key_flag as u32).wrapping_add(self.format.wrapping_mul(2))
    }
}

/// Where a texel read goes: a separate source buffer, or the destination
/// buffer itself (aliased source, §1 overlap rule).
enum Src<'a> {
    Separate(&'a [u8]),
    Aliased,
}

/// RenderBlt (§6) with a source buffer separate from the target.
pub fn render_blt(
    spans: &[Span],
    target: &mut Target,
    src_data: &[u8],
    src: &SourceImage,
    b: &BltParams,
    kind: u32,
) -> Result<(), SpanError> {
    blt(spans, target, Src::Separate(src_data), src, b, kind)
}

/// RenderBlt (§6) with the source inside the target's own buffer
/// (`src.origin` is an offset into `target.data`). Each pixel's texel is
/// read before its byte is written, in ascending x, span by span (§1), so
/// overlap behaves as an ascending byte-by-byte copy.
pub fn render_blt_aliased(
    spans: &[Span],
    target: &mut Target,
    src: &SourceImage,
    b: &BltParams,
    kind: u32,
) -> Result<(), SpanError> {
    blt(spans, target, Src::Aliased, src, b, kind)
}

fn blt(
    spans: &[Span],
    target: &mut Target,
    source: Src,
    src: &SourceImage,
    b: &BltParams,
    kind: u32,
) -> Result<(), SpanError> {
    let slot = Slot::new(b.row(), kind)?;
    let offs = target.check_spans(spans)?;
    let reads = body_reads_texel(slot);
    let geom = slot.geometry();
    // §7.3/§7.5: tiled bodies divide by W per span (the tiled fill too),
    // by H only when they read a texel. Checked up front even for row 13
    // with key 0 (draws nothing; no compatibility weight, §9).
    if geom == Geometry::Tiled && (src.width == 0 || (reads && src.height == 0)) {
        return Err(SpanError::ZeroTileSize);
    }
    let bpp = slot.bpp();
    let src_len = match &source {
        Src::Separate(d) => d.len(),
        Src::Aliased => target.data.len(),
    };
    // Pass 1: every texel address, checked before anything is written.
    let mut texels: Vec<usize> = Vec::new();
    if reads {
        for (i, s) in spans.iter().enumerate() {
            let y = s.y();
            let mut sx = 0u32;
            let mut sy = 0u32;
            if geom == Geometry::Tiled {
                sx = s.x().wrapping_add(b.u0) % src.width;
                sy = y.wrapping_add(b.v0) % src.height;
            }
            for k in 0..s.count() {
                let x = s.x() + k;
                let (tx, ty) = match geom {
                    Geometry::Plain => (x, y),
                    // §7.5: 32-bit wrapping sums, logical shifts.
                    Geometry::Stretch => (
                        b.u0.wrapping_add(b.du.wrapping_mul(x)) >> 16,
                        b.v0.wrapping_add(b.dv.wrapping_mul(y)) >> 16,
                    ),
                    Geometry::Tiled => {
                        let t = (sx, sy);
                        sx += 1;
                        if sx == src.width {
                            sx = 0;
                        }
                        t
                    }
                };
                let off = offset(
                    src.origin, src.stride, tx as u64, ty as u64, bpp as u64, bpp as u64, src_len,
                )
                .ok_or(SpanError::SourceOutOfBounds { span: i })?;
                texels.push(off);
            }
        }
    }
    // Pass 2: render.
    let ca = (b.colour >> 24) as u8;
    let f = slot.format();
    let mut next = texels.into_iter();
    for (s, off) in spans.iter().zip(offs) {
        for k in 0..s.count() as usize {
            let value = if !reads {
                // Fill bodies; row 13 ConstantAlpha draws only for key != 0.
                if slot.row == 13 && b.key == 0 {
                    None
                } else {
                    Some(0xFF)
                }
            } else {
                let t_off = next.next().expect("one texel per pixel");
                let data: &[u8] = match &source {
                    Src::Separate(d) => d,
                    Src::Aliased => target.data,
                };
                let mut t = 0u32;
                for j in 0..bpp as usize {
                    t |= (data[t_off + j] as u32) << (8 * j);
                }
                let draw = if slot.row == 13 {
                    // §7.4: row 13 compares no texel bits, only key != 0.
                    b.key != 0
                } else if slot.keyed() {
                    (t & KEY_MASKS[f as usize]) != b.key
                } else {
                    true
                };
                draw.then(|| column_value(slot.column(), texel_alpha(f, t), ca))
            };
            if let Some(v) = value {
                target.data[off + k] = v;
            }
        }
    }
    Ok(())
}
