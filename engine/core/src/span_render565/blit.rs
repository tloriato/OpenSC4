// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! The 565 blitter dispatch table (§7): row `2·f + key`, column `kind`,
//! and the bodies as one parameterised routine: source conversion (§7.2),
//! ConstantAlpha / Modulate / ModulateAlpha (§7.3), colour key (§7.3) and
//! plain / stretch / tiled addressing (§6.1, §7.4).

use super::{blend, pixel_addr, rows, BltParams, Dest, Mem, Source, Span, SpanError};

/// Table size (§6: 14 rows of 12 entries).
pub const TABLE_ROWS: u32 = 14;
pub const TABLE_COLUMNS: u32 = 12;

/// Body address of every slot, `0x008xxxxx` as listed in §7 (identity
/// only: slots with the same address run the same body). Used to check
/// that the parameterised bodies agree wherever the original folds them.
#[rustfmt::skip]
pub const SLOT_BODY: [[u32; 12]; 14] = [
    [0x846CD0, 0x83EF00, 0x846D80, 0x83EFD0, 0x846E70, 0x83F140, 0x846F30, 0x83F280, 0x847040, 0x83F420, 0x847110, 0x83F570],
    [0x847240, 0x83F710, 0x847310, 0x83F800, 0x847420, 0x83F990, 0x8474F0, 0x83FAE0, 0x847610, 0x83FC90, 0x8476F0, 0x83FE10],
    [0x840980, 0x83E2B0, 0x840AF0, 0x83FFD0, 0x840D50, 0x83E360, 0x840F50, 0x840140, 0x841200, 0x83E480, 0x841440, 0x8402E0],
    [0x8416F0, 0x83E5B0, 0x841890, 0x840480, 0x841B20, 0x83E670, 0x841D40, 0x840610, 0x842010, 0x83E7A0, 0x842290, 0x8407C0],
    [0x840980, 0x840A20, 0x840AF0, 0x840BE0, 0x840D50, 0x840E10, 0x840F50, 0x841070, 0x841200, 0x8412E0, 0x841440, 0x841560],
    [0x8416F0, 0x8417B0, 0x841890, 0x8419A0, 0x841B20, 0x841BF0, 0x841D40, 0x841E70, 0x842010, 0x842110, 0x842290, 0x8423C0],
    [0x848160, 0x842570, 0x842600, 0x842700, 0x833730, 0x842880, 0x842990, 0x842AB0, 0x842C50, 0x842CF0, 0x842E10, 0x842F40],
    [0x8430E0, 0x843160, 0x843200, 0x843320, 0x8434B0, 0x843550, 0x843670, 0x8437A0, 0x843950, 0x843A00, 0x843B40, 0x843C90],
    [0x844680, 0x83E8F0, 0x8447E0, 0x843E50, 0x844A00, 0x83E990, 0x844C00, 0x843F80, 0x844E50, 0x83EAB0, 0x845060, 0x8440E0],
    [0x8452B0, 0x83EBE0, 0x8454C0, 0x844240, 0x8456E0, 0x83EC90, 0x845910, 0x844390, 0x845B70, 0x83EDC0, 0x845DD0, 0x844500],
    [0x844680, 0x844720, 0x8447E0, 0x8448B0, 0x844A00, 0x844AC0, 0x844C00, 0x844CE0, 0x844E50, 0x844F20, 0x845060, 0x845140],
    [0x8452B0, 0x845370, 0x8454C0, 0x845590, 0x8456E0, 0x8457C0, 0x845910, 0x845A00, 0x845B70, 0x845C70, 0x845DD0, 0x845EE0],
    [0x847830, 0x846060, 0x847880, 0x8460D0, 0x847830, 0x846210, 0x847880, 0x8462C0, 0x847920, 0x846430, 0x847990, 0x846500],
    [0x847A50, 0x846670, 0x847B00, 0x8466F0, 0x847AA0, 0x846840, 0x847B00, 0x846900, 0x847BB0, 0x846A70, 0x847C20, 0x846B50],
];

/// Body address of slot `(row, kind)`, if inside the table.
pub fn body_address(row: u32, kind: u32) -> Option<u32> {
    (row < TABLE_ROWS && kind < TABLE_COLUMNS).then(|| SLOT_BODY[row as usize][kind as usize])
}

/// Source-format index `f` (§6.1) and its texel layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    A4444,
    A1555,
    X555,
    R565,
    A8888,
    X888,
    Index8,
}

impl SourceFormat {
    pub fn from_index(f: u32) -> Option<Self> {
        use SourceFormat::*;
        [A4444, A1555, X555, R565, A8888, X888, Index8].get(f as usize).copied()
    }
    /// Bytes per texel. X888 (colour type 8) is read as a 32-bit texel:
    /// its rows share every non-alpha body with A8888 (§7), and the key
    /// mask `0x00FFFFFF` (§6.1) and the §7.2 example are 32-bit values.
    pub fn bytes(self) -> usize {
        match self {
            SourceFormat::A8888 | SourceFormat::X888 => 4,
            SourceFormat::Index8 => 1,
            _ => 2,
        }
    }
    /// Key mask (§6.1 table `0x00830674`; 0 for the 8-bit source, §7.3).
    pub fn key_mask(self) -> u32 {
        match self {
            SourceFormat::A4444 => 0x0FFF,
            SourceFormat::A1555 | SourceFormat::X555 => 0x7FFF,
            SourceFormat::R565 => 0xFFFF,
            SourceFormat::A8888 | SourceFormat::X888 => 0x00FF_FFFF,
            SourceFormat::Index8 => 0,
        }
    }
}

/// Column `kind & 3` (§7).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BltOp {
    Blt,
    ConstantAlpha,
    Modulate,
    ModulateAlpha,
}

/// Column `kind >> 2` (§7: 0–3 plain, 4–7 stretch, 8–11 tiled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Addressing {
    Plain,
    Stretch,
    Tiled,
}

/// One slot of the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub format: SourceFormat,
    pub keyed: bool,
    pub op: BltOp,
    pub addressing: Addressing,
}

/// The slot at `(row, kind)`: row `2·f + key` (row 13, keyed 8-bit, is
/// unreachable from the shipped caller but present in the table).
pub fn slot(row: u32, kind: u32) -> Option<Slot> {
    if row >= TABLE_ROWS || kind >= TABLE_COLUMNS {
        return None;
    }
    Some(Slot {
        format: SourceFormat::from_index(row / 2)?,
        keyed: row & 1 == 1,
        op: [BltOp::Blt, BltOp::ConstantAlpha, BltOp::Modulate, BltOp::ModulateAlpha][(kind & 3) as usize],
        addressing: [Addressing::Plain, Addressing::Stretch, Addressing::Tiled][(kind >> 2) as usize],
    })
}

fn x5to8(v: u32) -> u32 {
    v << 3 | v >> 2
}

/// Source texel → 565 (§7.2).
pub fn texel_to_565(f: SourceFormat, t: u32) -> u16 {
    let v = match f {
        SourceFormat::A4444 => {
            let (r, g, b) = ((t >> 8) & 0xF, (t >> 4) & 0xF, t & 0xF);
            (r << 1 | r >> 3) << 11 | (g << 2 | g >> 2) << 5 | (b << 1 | b >> 3)
        }
        SourceFormat::A1555 | SourceFormat::X555 => {
            let g = (t >> 5) & 0x1F;
            (t & 0x7C00) << 1 | (g << 1 | g >> 4) << 5 | (t & 0x1F)
        }
        SourceFormat::R565 => t,
        SourceFormat::A8888 | SourceFormat::X888 => {
            ((t >> 19) & 0x1F) << 11 | ((t >> 10) & 0x3F) << 5 | ((t >> 3) & 0x1F)
        }
        SourceFormat::Index8 => 0xFFFF,
    };
    v as u16
}

/// The texel expanded to 8 bits per channel `(a, r, g, b)` (§7.3 Modulate).
fn texel_to_8888(f: SourceFormat, t: u32) -> [u32; 4] {
    match f {
        SourceFormat::A4444 => [(t >> 12) & 0xF, (t >> 8) & 0xF, (t >> 4) & 0xF, t & 0xF].map(|x| x * 17),
        SourceFormat::A1555 | SourceFormat::X555 => {
            // A1555's 1-bit alpha expanded as 0 → 0, 1 → 255: §7.3 says
            // "expand the texel to 8 bits per channel" without spelling out
            // the 1-bit case (question SR565-2).
            let a = match f {
                SourceFormat::A1555 if t & 0x8000 == 0 => 0,
                _ => 255,
            };
            [a, x5to8((t >> 10) & 0x1F), x5to8((t >> 5) & 0x1F), x5to8(t & 0x1F)]
        }
        SourceFormat::R565 => {
            let g = (t >> 5) & 0x3F;
            [255, x5to8(t >> 11 & 0x1F), g << 2 | g >> 4, x5to8(t & 0x1F)]
        }
        SourceFormat::A8888 => [t >> 24, (t >> 16) & 0xFF, (t >> 8) & 0xFF, t & 0xFF],
        SourceFormat::X888 => [255, (t >> 16) & 0xFF, (t >> 8) & 0xFF, t & 0xFF],
        SourceFormat::Index8 => [t & 0xFF, 255, 255, 255],
    }
}

/// Modulated `(packed 565, mA)` (§7.3): `m = (c·cc + 128) >> 8`.
fn modulate(f: SourceFormat, t: u32, argb: u32) -> (u16, u32) {
    let c = texel_to_8888(f, t);
    let cc = [argb >> 24, (argb >> 16) & 0xFF, (argb >> 8) & 0xFF, argb & 0xFF];
    let m: [u32; 4] = std::array::from_fn(|i| (c[i] * cc[i] + 128) >> 8);
    (((m[1] >> 3) << 11 | (m[2] >> 2) << 5 | (m[3] >> 3)) as u16, m[0])
}

/// Whether the slot reads the texel at all. The 8-bit Blt and Modulate
/// bodies ignore it (§7: their plain and stretch slots are one body), and
/// the 8-bit ConstantAlpha colour is white whatever the texel (§7.2), and
/// the keyed 8-bit row compares no texel bits (§7.3), so only the 8-bit
/// ModulateAlpha needs it. A read is only observable through a fault, which
/// the core turns into a bounds error, so this choice decides only which
/// out-of-arena inputs are rejected.
fn reads_texel(s: &Slot) -> bool {
    s.format != SourceFormat::Index8 || s.op == BltOp::ModulateAlpha
}

/// Runs one body over the span list (§7.1). Per pixel: texel, then (when
/// blending) destination, then the write, so aliased buffers behave as an
/// ascending element-by-element copy (§1).
pub(crate) fn run(
    m: &mut Mem<'_>,
    s: Slot,
    spans: &[Span],
    dst: &Dest,
    src: &Source,
    b: &BltParams,
) -> Result<(), SpanError> {
    let list = rows(spans)?;
    if s.addressing == Addressing::Tiled && (src.width == 0 || src.height == 0) {
        return Err(SpanError::TiledZeroSize);
    }
    let bpp = s.format.bytes() as i64;
    let stride = src.stride as i64;
    let mask = s.format.key_mask();
    let ca = b.argb >> 24;
    for (x, y, count) in list {
        // Texel row and first column for this span.
        let (ty, mut sx) = match s.addressing {
            Addressing::Plain => (y, x),
            // Logical shifts on 32-bit wrapping sums (§7.4).
            Addressing::Stretch => (b.v0.wrapping_add(b.dv.wrapping_mul(y)) >> 16, 0),
            Addressing::Tiled => (y.wrapping_add(b.v0) % src.height, x.wrapping_add(b.u0) % src.width),
        };
        for k in 0..count {
            let px = x + k;
            let tx = match s.addressing {
                Addressing::Plain => px,
                Addressing::Stretch => b.u0.wrapping_add(b.du.wrapping_mul(px)) >> 16,
                Addressing::Tiled => sx,
            };
            if s.addressing == Addressing::Tiled {
                sx += 1;
                if sx == src.width {
                    sx = 0;
                }
            }
            let t = if reads_texel(&s) {
                m.read(src.base + ty as i64 * stride + tx as i64 * bpp, s.format.bytes())?
            } else {
                0
            };
            if s.keyed && t & mask == b.key {
                continue;
            }
            let addr = pixel_addr(dst, px, y);
            let out = match s.op {
                BltOp::Blt => texel_to_565(s.format, t),
                BltOp::Modulate => modulate(s.format, t, b.argb).0,
                BltOp::ConstantAlpha => {
                    // §7.3: weight ca >> 3 for every format; the A-format
                    // rows have their own ConstantAlpha bodies yet the spec
                    // gives no use of the texel alpha (question SR565-3).
                    let d = m.read16(addr)?;
                    blend(texel_to_565(s.format, t), d, ca >> 3)
                }
                BltOp::ModulateAlpha => {
                    let (p, ma) = modulate(s.format, t, b.argb);
                    let d = m.read16(addr)?;
                    blend(p, d, ma >> 3)
                }
            };
            m.write16(addr, out)?;
        }
    }
    Ok(())
}
