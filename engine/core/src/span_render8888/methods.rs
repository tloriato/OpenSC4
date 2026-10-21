// SPDX-License-Identifier: GPL-3.0-or-later
//! The four span methods of `cSpanRender8888` (§3-§6).

use super::{blend, check_dest, Span, SpanError, Surface};

fn put(buf: &mut [u8], off: usize, v: u32) {
    buf[off..off + 4].copy_from_slice(&v.to_le_bytes());
}

fn get(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes([buf[off], buf[off + 1], buf[off + 2], buf[off + 3]])
}

/// RenderSolid (§3): every pixel of every span becomes `colour`, verbatim
/// (alpha byte included). Width 0 writes nothing.
pub fn render_solid(spans: &[Span], buf: &mut [u8], dst: Surface, colour: u32) -> Result<(), SpanError> {
    let starts = check_dest(spans, dst, buf.len())?;
    for (s, &off) in spans.iter().zip(&starts) {
        for i in 0..s.width() as usize {
            put(buf, off + 4 * i, colour);
        }
    }
    Ok(())
}

/// RenderAlpha weight (§4): `(alpha >> 24) + (alpha >> 31)`, 0..256.
pub fn render_alpha_weight(alpha: u32) -> u32 {
    (alpha >> 24) + (alpha >> 31)
}

/// RenderAlpha (§4): each pixel becomes `B(d, colour, w)`, output alpha `0xFF`.
pub fn render_alpha(spans: &[Span], buf: &mut [u8], dst: Surface, colour: u32, alpha: u32) -> Result<(), SpanError> {
    let starts = check_dest(spans, dst, buf.len())?;
    let w = render_alpha_weight(alpha);
    for (s, &off) in spans.iter().zip(&starts) {
        for i in 0..s.width() as usize {
            let o = off + 4 * i;
            let d = get(buf, o);
            put(buf, o, blend(d, colour, w));
        }
    }
    Ok(())
}

/// Gradient record (§5, §6): signed 8.16 fixed point, bits 16-23 the 8-bit
/// channel. `w*` (the `+0x0C/+0x1C/+0x2C` dwords) is read only by
/// RenderAlphaGradient. Index order of each array: B, G, R, W.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Gradient {
    /// Start values at (0, 0): `+0x00` B0, `+0x04` G0, `+0x08` R0, `+0x0C` W0.
    pub start: [u32; 4],
    /// x steps: `+0x10`..`+0x1C`.
    pub dx: [u32; 4],
    /// y steps: `+0x20`..`+0x2C`.
    pub dy: [u32; 4],
}

impl Gradient {
    /// From the 12 dwords at `grad+0x00..+0x2C`.
    pub fn from_dwords(d: &[u32; 12]) -> Gradient {
        let a = |i: usize| [d[i], d[i + 1], d[i + 2], d[i + 3]];
        Gradient {
            start: a(0),
            dx: a(4),
            dy: a(8),
        }
    }
    /// Accumulators at the span start (x, y): `v0 + x*dx + y*dy`, 32-bit wrap.
    fn at(&self, x: u32, y: u32) -> [u32; 4] {
        let mut acc = [0u32; 4];
        for (c, a) in acc.iter_mut().enumerate() {
            *a = self.start[c]
                .wrapping_add(x.wrapping_mul(self.dx[c]))
                .wrapping_add(y.wrapping_mul(self.dy[c]));
        }
        acc
    }
    fn step(&self, acc: &mut [u32; 4]) {
        for (c, a) in acc.iter_mut().enumerate() {
            *a = a.wrapping_add(self.dx[c]);
        }
    }
}

fn byte2(v: u32) -> u32 {
    (v >> 16) & 0xFF
}

/// RenderGradient (§5): opaque ramp. accR gets `0xFF000000` ORed in at the
/// span start; each pixel is `(accR & 0xFFFF0000) | byte2(G) << 8 | byte2(B)`,
/// so an R carry or borrow out of bit 23 changes the alpha byte. No clamping.
pub fn render_gradient(spans: &[Span], buf: &mut [u8], dst: Surface, grad: &Gradient) -> Result<(), SpanError> {
    let starts = check_dest(spans, dst, buf.len())?;
    for (s, &off) in spans.iter().zip(&starts) {
        let mut acc = grad.at(s.x(), s.y());
        acc[2] |= 0xFF00_0000;
        for i in 0..s.width() as usize {
            put(
                buf,
                off + 4 * i,
                (acc[2] & 0xFFFF_0000) | byte2(acc[1]) << 8 | byte2(acc[0]),
            );
            grad.step(&mut acc);
        }
    }
    Ok(())
}

/// RenderAlphaGradient weight (§6): `accW >> 16`, unmasked, 0..65535.
pub fn alpha_gradient_weight(acc: u32) -> u32 {
    acc >> 16
}

/// The packed blend of RenderAlphaGradient (§6), exact for every `w` (equal
/// to `B(d, s, w)` for `w <= 256`; the fields interfere above that).
pub fn blend_alpha_gradient(d: u32, s: u32, w: u32) -> u32 {
    let (s_rb, d_rb) = (s & 0x00FF_00FF, d & 0x00FF_00FF);
    let (s_g, d_g) = (s & 0xFF00, d & 0xFF00);
    let rb = (s_rb.wrapping_sub(d_rb).wrapping_mul(w).wrapping_add(0x0080_0080) >> 8).wrapping_add(d_rb);
    let g = (s_g.wrapping_sub(d_g) >> 8)
        .wrapping_mul(w)
        .wrapping_add(d_g)
        .wrapping_add(0x80);
    (rb | 0xFF00_FF00) & (g | 0xFFFF_00FF)
}

/// RenderAlphaGradient (§6): source colour from the R, G, B accumulators,
/// weight from W, pixel = packed blend, output alpha `0xFF`. (The original
/// adds `0xFF000000` to accR; it does not reach the output and is omitted.)
pub fn render_alpha_gradient(spans: &[Span], buf: &mut [u8], dst: Surface, grad: &Gradient) -> Result<(), SpanError> {
    let starts = check_dest(spans, dst, buf.len())?;
    for (s, &off) in spans.iter().zip(&starts) {
        let mut acc = grad.at(s.x(), s.y());
        for i in 0..s.width() as usize {
            let o = off + 4 * i;
            let src = byte2(acc[2]) << 16 | byte2(acc[1]) << 8 | byte2(acc[0]);
            let d = get(buf, o);
            put(buf, o, blend_alpha_gradient(d, src, alpha_gradient_weight(acc[3])));
            grad.step(&mut acc);
        }
    }
    Ok(())
}
