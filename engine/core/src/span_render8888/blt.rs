// SPDX-License-Identifier: GPL-3.0-or-later
//! RenderBlt and its 14 x 12 kernel table (§7-§9).

use super::{blend, check_dest, offset, Mem, Span, SpanError, Surface};

/// Source pixel formats, `src.format` 0..6 (§8).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceFormat {
    A4R4G4B4 = 0,
    A1R5G5B5 = 1,
    X1R5G5B5 = 2,
    R5G6B5 = 3,
    A8R8G8B8 = 4,
    X8R8G8B8 = 5,
    A8 = 6,
}

impl SourceFormat {
    pub const ALL: [SourceFormat; 7] = [
        SourceFormat::A4R4G4B4,
        SourceFormat::A1R5G5B5,
        SourceFormat::X1R5G5B5,
        SourceFormat::R5G6B5,
        SourceFormat::A8R8G8B8,
        SourceFormat::X8R8G8B8,
        SourceFormat::A8,
    ];
    pub fn from_u32(v: u32) -> Option<SourceFormat> {
        SourceFormat::ALL.get(v as usize).copied()
    }
    /// Bytes per source texel.
    pub fn bpp(self) -> usize {
        match self {
            SourceFormat::A8R8G8B8 | SourceFormat::X8R8G8B8 => 4,
            SourceFormat::A8 => 1,
            _ => 2,
        }
    }
}

/// Colour-key masks by format (§9.2), applied to the raw texel. A8 has no
/// colour bits and is keyed by `key == 0` instead (see [`keyed_skip`]).
pub const KEY_MASKS: [u32; 7] = [0x0FFF, 0x7FFF, 0x7FFF, 0xFFFF, 0x00FF_FFFF, 0x00FF_FFFF, 0];

/// Number of kernels: 14 rows x 12 columns (§7).
pub const KERNEL_COUNT: usize = 168;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Geometry {
    Direct = 0,
    Stretch = 1,
    Tiled = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Op {
    Copy = 0,
    ConstantAlpha = 1,
    Modulate = 2,
    ModulateAlpha = 3,
}

/// The 32-byte source descriptor `src` (§7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BltDesc {
    pub u0: u32,
    pub v0: u32,
    pub du: u32,
    pub dv: u32,
    pub format: u32,
    /// Constant colour `K` (A8R8G8B8), `+0x14`.
    pub k: u32,
    /// Colour key, `+0x18`.
    pub key: u32,
    /// Keyed flag byte, `+0x1C`.
    pub keyed: u8,
}

impl BltDesc {
    /// From the 32 descriptor bytes (little endian); `+0x1D..+0x1F` unread.
    pub fn from_bytes(b: &[u8; 32]) -> BltDesc {
        let d = |o: usize| u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]]);
        BltDesc {
            u0: d(0),
            v0: d(4),
            du: d(8),
            dv: d(12),
            format: d(16),
            k: d(20),
            key: d(24),
            keyed: b[28],
        }
    }
}

/// Source image (§7 `rectB`): byte offset of texel (0, 0) within the source
/// bytes, signed stride in bytes, width and height (read by tiled kernels only).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceImage {
    pub base: usize,
    pub stride: i32,
    pub width: u32,
    pub height: u32,
}

/// One of the 168 kernels: row `format*2 + keyed`, column `4*geometry + op`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Kernel {
    pub format: SourceFormat,
    pub keyed: bool,
    pub geometry: Geometry,
    pub op: Op,
}

impl Kernel {
    /// Kernel at table row `row` (0..14), column `mode` (0..12).
    pub fn from_index(row: u32, mode: u32) -> Option<Kernel> {
        if row >= 14 || mode >= 12 {
            return None;
        }
        let geometry = [Geometry::Direct, Geometry::Stretch, Geometry::Tiled][(mode / 4) as usize];
        let op = [Op::Copy, Op::ConstantAlpha, Op::Modulate, Op::ModulateAlpha][(mode % 4) as usize];
        Some(Kernel {
            format: SourceFormat::ALL[(row / 2) as usize],
            keyed: row % 2 == 1,
            geometry,
            op,
        })
    }
    pub fn row(self) -> u32 {
        self.format as u32 * 2 + self.keyed as u32
    }
    pub fn mode(self) -> u32 {
        4 * self.geometry as u32 + self.op as u32
    }
    /// RenderBlt's dispatch (§7): `table[format*2 + keyed][mode]`. The original
    /// checks neither index (U2); this API rejects `format > 6`, a keyed byte
    /// other than 0/1 (literally `format*2 + keyed` would select a neighbouring
    /// format's row; question SR8888-2) and `mode > 11`.
    pub fn dispatch(desc: &BltDesc, mode: u32) -> Result<Kernel, SpanError> {
        let bad = SpanError::BadKernelIndex {
            format: desc.format,
            keyed: desc.keyed,
            mode,
        };
        if desc.format > 6 || desc.keyed > 1 {
            return Err(bad);
        }
        Kernel::from_index(desc.format * 2 + desc.keyed as u32, mode).ok_or(bad)
    }
}

/// Widening `C(p)` of a raw texel to A8R8G8B8 (§8).
pub fn widen(fmt: SourceFormat, p: u32) -> u32 {
    let five = |v: u32| (v << 3) | (v >> 2);
    let rgb555 = |p: u32| five((p >> 10) & 0x1F) << 16 | five((p >> 5) & 0x1F) << 8 | five(p & 0x1F);
    match fmt {
        SourceFormat::A4R4G4B4 => (0..4).fold(0, |v, i| v | (((p >> (4 * i)) & 0xF) * 17) << (8 * i)),
        SourceFormat::A1R5G5B5 => (if p & 0x8000 != 0 { 0xFF00_0000 } else { 0 }) | rgb555(p),
        SourceFormat::X1R5G5B5 => 0xFF00_0000 | rgb555(p),
        SourceFormat::R5G6B5 => {
            let g = (p >> 5) & 0x3F;
            0xFF00_0000 | five((p >> 11) & 0x1F) << 16 | ((g << 2) | (g >> 4)) << 8 | five(p & 0x1F)
        }
        SourceFormat::A8R8G8B8 => p,
        SourceFormat::X8R8G8B8 => (p & 0x00FF_FFFF) | 0xFF00_0000,
        SourceFormat::A8 => (p << 24) | 0x00FF_FFFF,
    }
}

/// Keyed rows (§9.2): true if the raw texel `p` leaves the destination pixel
/// untouched. A8 tests only `key == 0` (then nothing is drawn).
pub fn keyed_skip(fmt: SourceFormat, p: u32, key: u32) -> bool {
    match fmt {
        SourceFormat::A8 => key == 0,
        _ => p & KEY_MASKS[fmt as usize] == key,
    }
}

/// Modulate (§9.2 op 2): per byte, alpha included, `(C*K + 128) >> 8`.
pub fn modulate(c: u32, k: u32) -> u32 {
    (0..4).fold(0, |v, i| {
        let (cb, kb) = ((c >> (8 * i)) & 0xFF, (k >> (8 * i)) & 0xFF);
        v | ((cb * kb + 128) >> 8) << (8 * i)
    })
}

/// Walks the texel byte offsets of one span (§9.1); `None` = outside the
/// source bytes.
struct Texels {
    geometry: Geometry,
    src: SourceImage,
    bpp: i64,
    len: usize,
    // Direct: (x, y) texel; stretch: u accumulator and row v; tiled: (tx, ty).
    a: u32,
    b: u32,
    du: u32,
    x: i64,
}

impl Texels {
    fn new(k: Kernel, src: SourceImage, desc: &BltDesc, span: Span, len: usize) -> Texels {
        let (x, y) = (span.x(), span.y());
        let (a, b) = match k.geometry {
            Geometry::Direct => (0, y),
            Geometry::Stretch => (
                desc.du.wrapping_mul(x).wrapping_add(desc.u0),
                desc.dv.wrapping_mul(y).wrapping_add(desc.v0) >> 16,
            ),
            // Width/height 0 is rejected before any walk (ZeroTileSize).
            Geometry::Tiled => (
                x.wrapping_add(desc.u0) % src.width,
                y.wrapping_add(desc.v0) % src.height,
            ),
        };
        Texels {
            geometry: k.geometry,
            src,
            bpp: k.format.bpp() as i64,
            len,
            a,
            b,
            du: desc.du,
            x: x as i64,
        }
    }
    fn next(&mut self) -> Option<usize> {
        let s = self.src;
        let (tx, ty) = match self.geometry {
            Geometry::Direct => {
                self.x += 1;
                (self.x - 1, self.b as i64)
            }
            Geometry::Stretch => {
                let t = self.a >> 16;
                self.a = self.a.wrapping_add(self.du);
                (t as i64, self.b as i64)
            }
            Geometry::Tiled => {
                let t = self.a;
                self.a += 1;
                if self.a == s.width {
                    self.a = 0;
                }
                (t as i64, self.b as i64)
            }
        };
        offset(s.base, s.stride, tx, ty, self.bpp, self.len, self.bpp as usize)
    }
}

/// RenderBlt (§7): dispatches on `(desc.format, desc.keyed, mode)` and runs
/// the kernel. `mem` holds both destination and source (see [`Mem`]).
pub fn render_blt(
    spans: &[Span],
    mem: &mut Mem,
    dst: Surface,
    src: SourceImage,
    desc: &BltDesc,
    mode: u32,
) -> Result<(), SpanError> {
    let k = Kernel::dispatch(desc, mode)?;
    run_kernel(k, spans, mem, dst, src, desc)
}

/// Runs one kernel (§9). The kernel's own row decides format and keying;
/// `desc.format`/`desc.keyed` are not re-read (the original's kernels are
/// separate functions per row).
pub fn run_kernel(
    k: Kernel,
    spans: &[Span],
    mem: &mut Mem,
    dst: Surface,
    src: SourceImage,
    desc: &BltDesc,
) -> Result<(), SpanError> {
    let starts = check_dest(spans, dst, mem.dst_len())?;
    if k.geometry == Geometry::Tiled && (src.width == 0 || src.height == 0) {
        return Err(SpanError::ZeroTileSize);
    }
    // Validation pass: every texel address. Choice: also done for keyed A8
    // with key 0, which reads nothing in the original (§9.2); the walk is
    // bounded because check_dest has bounded every width by the target size.
    let slen = mem.src_len();
    for (i, s) in spans.iter().enumerate() {
        let mut t = Texels::new(k, src, desc, *s, slen);
        for _ in 0..s.width() {
            t.next().ok_or(SpanError::SourceOutOfBounds { span: i })?;
        }
    }
    let bpp = k.format.bpp();
    for (s, &off) in spans.iter().zip(&starts) {
        let mut t = Texels::new(k, src, desc, *s, slen);
        for i in 0..s.width() as usize {
            let toff = t.next().expect("validated");
            if k.keyed && k.format == SourceFormat::A8 && desc.key == 0 {
                continue;
            }
            // Read order per pixel: texel, then destination, then write; all
            // against live memory, so overlapping source/destination sees
            // pixels already written earlier in this call.
            let p = mem.read_src(toff, bpp);
            if k.keyed && keyed_skip(k.format, p, desc.key) {
                continue;
            }
            let c = widen(k.format, p);
            let o = off + 4 * i;
            let out = match k.op {
                Op::Copy => c,
                Op::ConstantAlpha => blend(mem.read_dst(o), c, desc.k >> 24),
                Op::Modulate => modulate(c, desc.k),
                Op::ModulateAlpha => {
                    let m = modulate(c, desc.k);
                    blend(mem.read_dst(o), m, m >> 24)
                }
            };
            mem.write_dst(o, out);
        }
    }
    Ok(())
}
