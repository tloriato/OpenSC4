//! Tests from the public spec `docs/specs/span-render-1555.md` only.

use super::*;

fn surf(buf: &mut [u8], w: i32) -> Surface<'_> {
    Surface {
        buf,
        base: 0,
        stride: 2 * w,
    }
}

fn px(buf: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([buf[2 * i], buf[2 * i + 1]])
}

fn fill(buf: &mut [u8], v: u16) {
    for c in buf.chunks_mut(2) {
        c.copy_from_slice(&v.to_le_bytes());
    }
}

fn per_channel(s: u16, d: u16, w: u32) -> u16 {
    let ch = |v: u16, sh: u32| u32::from(v >> sh) & 31;
    let c = |sh: u32| ((32 - w) * ch(d, sh) + w * ch(s, sh) + 16) / 32;
    (0x8000 | (c(10) << 10) | (c(5) << 5) | c(0)) as u16
}

#[test]
fn blend_packed_equals_per_channel_for_w_up_to_32() {
    let mut x: u32 = 12345;
    for w in 0..=32 {
        for _ in 0..400 {
            x = x.wrapping_mul(1_103_515_245).wrapping_add(12345);
            let (s, d) = ((x >> 16) as u16, x as u16);
            assert_eq!(blend(s, d, w), per_channel(s, d, w), "s={s:#x} d={d:#x} w={w}");
        }
    }
}

#[test]
fn blend_above_32_is_the_packed_form() {
    // §5: independent recomputation of the packed form in 64-bit, mod 2^32.
    for (s, d, w) in [(0x7FFFu16, 0u16, 63u32), (0, 0x7FFF, 40), (0x1234, 0x6543, 33)] {
        let m = |v: i64| (v.rem_euclid(1 << 32) as u64) >> 5;
        let (sp, dp) = (i64::from(s & 0x7C1F), i64::from(d & 0x7C1F));
        let (sg, dg) = (i64::from(s & 0x03E0), i64::from(d & 0x03E0));
        let rb = m((sp - dp) * i64::from(w) + 32 * dp + 0x4010);
        let g = m((sg - dg) * i64::from(w) + 32 * (dg + 0x10));
        assert_eq!(blend(s, d, w), ((rb | 0x83E0) & (g | 0xFC1F)) as u16);
    }
}

#[test]
fn alpha_weight_thresholds() {
    let t = [
        8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 87, 95, 103, 111, 119, 127, 135, 143, 151, 159, 167, 174, 182, 190, 198,
        206, 214, 222, 230, 238, 246, 254,
    ];
    for (i, &a) in t.iter().enumerate() {
        let w = i as u32 + 1;
        assert_eq!(alpha_weight(a << 24), w);
        assert_eq!(alpha_weight((a - 1) << 24), w - 1);
    }
    assert_eq!(alpha_weight(0xFF00_0000), 32);
}

#[test]
fn alpha_gradient_weight_thresholds() {
    let t = [
        8, 16, 24, 32, 39, 47, 55, 63, 70, 78, 86, 94, 101, 109, 117, 125, 132, 140, 148, 156, 163, 171, 179, 187, 194,
        202, 210, 218, 225, 233, 241, 249,
    ];
    let w = |a: u32| (a << 16).wrapping_mul(0x84) >> 26;
    for (i, &a) in t.iter().enumerate() {
        assert_eq!(w(a), i as u32 + 1);
        assert_eq!(w(a - 1), i as u32);
    }
}

#[test]
fn render_alpha_worked_example() {
    let mut b = vec![0u8; 8];
    render_alpha(&[Span::row(0, 0, 4)], &mut surf(&mut b, 4), 0x7FFF, 0x8000_0000).unwrap();
    assert!((0..4).all(|i| px(&b, i) == 0xC210));
    // a8 ≤ 7: no colour change, bit 15 still set; src bit 15 ignored.
    fill(&mut b, 0x1234);
    render_alpha(&[Span::row(0, 0, 4)], &mut surf(&mut b, 4), 0xFFFF, 0x07FF_FFFF).unwrap();
    assert_eq!(px(&b, 0), 0x9234);
}

#[test]
fn solid_and_preconditions() {
    let mut b = vec![0u8; 4 * 3 * 2];
    let spans = [Span::row(1, 1, 2), Span::row(3, 2, 0)];
    render_solid(&spans, &mut surf(&mut b, 4), 0xABCD_8001).unwrap();
    let got: Vec<u16> = (0..12).map(|i| px(&b, i)).collect();
    assert_eq!(&got[4..8], &[0, 0x8001, 0x8001, 0]);
    assert_eq!(got.iter().filter(|&&v| v != 0).count(), 2);
    let s = &mut surf(&mut b, 4);
    assert_eq!(render_solid(&[], s, 1), Err(Error::EmptySpanList));
    let cross = Span {
        start: 0,
        end: 0x0001_0001,
    };
    assert_eq!(render_solid(&[cross], s, 1), Err(Error::InvalidSpan { index: 0 }));
    let back = Span { start: 3, end: 2 };
    assert_eq!(render_solid(&[back], s, 1), Err(Error::InvalidSpan { index: 0 }));
    let oob = [Span::row(0, 0, 1), Span::row(2, 2, 3)];
    assert_eq!(render_solid(&oob, s, 7), Err(Error::DestOutOfBounds { index: 1 }));
    assert_eq!(px(s.buf, 0), 0, "nothing written before the error");
}

#[test]
fn overlapping_spans_blend_twice() {
    let mut b = vec![0u8; 2];
    let sp = Span::row(0, 0, 1);
    render_alpha(&[sp, sp], &mut surf(&mut b, 1), 0x7FFF, 0x8000_0000).unwrap();
    assert_eq!(px(&b, 0), per_channel(0x7FFF, 0xC210, 16));
}

#[test]
fn gradient_producer_worked_example() {
    let mut p = [0f32; 12];
    p[0] = 1.0;
    let g = gradient_from_transform(Some(&p), 0.0, 0.0).unwrap();
    assert_eq!(g.c[0], 0x00FF_8000);
    assert_eq!((g.dx[0], g.dy[0]), (0, 0));
    assert_eq!(g.c[1], 0x8000);
    assert_eq!(gradient_from_transform(None, 0.0, 0.0), None);
    let mut b = vec![0u8; 2];
    render_gradient(&[Span::row(0, 0, 1)], &mut surf(&mut b, 1), &g).unwrap();
    assert_eq!(px(&b, 0), 31);
    // (x0, y0) is the pixel where the colour equals p: c = s − dx·x0 − dy·y0.
    p[4] = 1.0 / 255.0;
    let g = gradient_from_transform(Some(&p), 4.0, 0.0).unwrap();
    let dx = g.dx[0];
    assert_eq!(dx, 65536);
    assert_eq!(g.c[0], 0x00FF_8000 - 4 * 65536);
}

#[test]
fn gradient_start_not_shifted_step_shifted() {
    let g = GradientRecord {
        c: [0x001F_0000, 0x03E0_0000, 0x7C00_0000, 0],
        dx: [0x10000, 0x10000, 0x10000, 0],
        dy: [0, 0, 0, 0],
    };
    let mut b = vec![0u8; 6];
    render_gradient(&[Span::row(0, 0, 3)], &mut surf(&mut b, 3), &g).unwrap();
    // k = 0: start bits 16-20 / 21-25 / 26-30 all ones.
    assert_eq!(px(&b, 0), 0x7FFF);
    // k = 1: B wraps 31→0, G += 32<<16 → +1 at bit 21, R += 1024<<16 → +1 at bit 26 (no clamp).
    assert_eq!(px(&b, 1), 0);
    assert_eq!(px(&b, 2), (1 << 10) | (1 << 5) | 1);
}

#[test]
fn gradient_uses_absolute_coordinates() {
    let g = GradientRecord {
        c: [0, 0, 0, 0],
        dx: [0x10000, 0, 0, 0],
        dy: [0x20000, 0, 0, 0],
    };
    let mut b = vec![0u8; 4 * 4 * 2];
    render_gradient(&[Span::row(2, 3, 2)], &mut surf(&mut b, 4), &g).unwrap();
    assert_eq!((px(&b, 14), px(&b, 15)), (8, 9));
}

#[test]
fn alpha_gradient_b_step_truncated() {
    // dx0 = 0x7 (low 3 bits lost per pixel): B accumulator never moves.
    let g = GradientRecord {
        c: [0x0007_FFFF << 3, 0, 0, 255 << 16],
        dx: [7, 0, 0, 0],
        dy: [0; 4],
    };
    let mut b = vec![0u8; 2 * 40];
    render_alpha_gradient(&[Span::row(0, 0, 40)], &mut surf(&mut b, 40), &g).unwrap();
    // V_0(x) = 0x3FFFF8 + 7x crosses 2^22 (B=8 → ...) but the truncated step keeps B = 7.
    assert!((0..40).all(|i| px(&b, i) & 31 == 7), "{:#x}", px(&b, 39));
}

#[test]
fn alpha_gradient_weight_above_32() {
    let g = GradientRecord {
        c: [0, 0, 0x00FF_0000, 0x01F0_0000],
        dx: [0; 4],
        dy: [0; 4],
    };
    let w = 0x01F0_0000u32.wrapping_mul(0x84) >> 26;
    assert!(w > 32);
    let mut b = vec![0u8; 2];
    fill(&mut b, 0x0421);
    render_alpha_gradient(&[Span::row(0, 0, 1)], &mut surf(&mut b, 1), &g).unwrap();
    assert_eq!(px(&b, 0), blend(31 << 10, 0x0421, w));
}

fn src16(texels: &[u16]) -> Vec<u8> {
    texels.iter().flat_map(|t| t.to_le_bytes()).collect()
}

fn blt(dst: &mut [u8], w: i32, src: &BltSource, b: &BlitRecord, kind: u32, spans: &[Span]) {
    render_blt(spans, &mut surf(dst, w), src, b, kind).unwrap();
}

fn source(buf: &[u8], stride: i32, w: u32, h: u32) -> BltSource<'_> {
    BltSource {
        buf,
        base: 0,
        stride,
        width: w,
        height: h,
        color_type: 0,
        bpp: 0,
    }
}

#[test]
fn conversions_column0() {
    assert_eq!(convert(0, 0xF8C1), 0x8000 | (0x11 << 10) | (0x19 << 5) | 0x02);
    assert_eq!(convert(0, 0x7FFF), 0x7FFF);
    assert_eq!(convert(1, 0x0001), 0x0001);
    assert_eq!(convert(2, 0x1234), 0x9234);
    assert_eq!(convert(3, 0xFFFF), 0xFFFF);
    assert_eq!(convert(3, 0x07E0), 0x8000 | (31 << 5));
    assert_eq!(convert(3, 0x0020), 0x8000, "lowest green bit dropped");
    assert_eq!(convert(4, 0x7FFF_FFFF), 0x7FFF);
    assert_eq!(convert(4, 0x8007_0F17), 0x8000 | (1 << 5) | 2);
    assert_eq!(convert(5, 0x00FF_FFFF), 0xFFFF);
    assert_eq!(convert(6, 0x7F), 0x7FFF);
    assert_eq!(convert(6, 0x80), 0xFFFF);
}

#[test]
fn expansions() {
    assert_eq!(expand(0, 0xF1A5), [255, 17, 170, 85]);
    assert_eq!(expand(1, 0x7FFF), [0, 255, 255, 255]);
    assert_eq!(expand(1, 0x8421), [255, 8, 8, 8]);
    assert_eq!(expand(2, 0x0000), [255, 0, 0, 0]);
    assert_eq!(expand(3, 0x07E0), [255, 0, 255, 0]);
    assert_eq!(expand(3, 0x0020), [255, 0, 4, 0]);
    assert_eq!(expand(4, 0x1122_3344), [0x11, 0x22, 0x33, 0x44]);
    assert_eq!(expand(5, 0x1122_3344), [0xFF, 0x22, 0x33, 0x44]);
    assert_eq!(expand(6, 0x40), [0x40, 255, 255, 255]);
    assert_eq!(modulate([255; 4], 0xFFFF_FFFF), [254; 4]);
    assert_eq!(modulate([255, 0, 128, 255], 0x8080_8080), [128, 0, 64, 128]);
}

#[test]
fn row2_raw_copy_and_row3_key_ignores_alpha() {
    let s = src16(&[0x8001, 0x0002, 0x8005, 0x7FFF]);
    let src = source(&s, 8, 4, 1);
    let mut d = vec![0u8; 8];
    fill(&mut d, 0x1111);
    let b = BlitRecord {
        format: 1,
        ..Default::default()
    };
    blt(&mut d, 4, &src, &b, 0, &[Span::row(0, 0, 4)]);
    assert_eq!(
        (0..4).map(|i| px(&d, i)).collect::<Vec<_>>(),
        [0x8001, 2, 0x8005, 0x7FFF]
    );
    fill(&mut d, 0x1111);
    let b = BlitRecord {
        format: 1,
        key_flag: 1,
        key: 0x0005,
        ..Default::default()
    };
    blt(&mut d, 4, &src, &b, 0, &[Span::row(0, 0, 4)]);
    assert_eq!(
        (0..4).map(|i| px(&d, i)).collect::<Vec<_>>(),
        [0x8001, 2, 0x1111, 0x7FFF]
    );
}

#[test]
fn keyed_rows_mask_alpha_and_row13() {
    // A4444 key compares the low 12 bits only.
    let s = src16(&[0xF123, 0x0123, 0x0124]);
    let src = source(&s, 6, 3, 1);
    let mut d = vec![0u8; 6];
    fill(&mut d, 0x4444);
    let b = BlitRecord {
        format: 0,
        key_flag: 1,
        key: 0x123,
        color: 0xFFFF_FFFF,
        ..Default::default()
    };
    for kind in 0..4 {
        blt(&mut d, 3, &src, &b, kind, &[Span::row(0, 0, 3)]);
        assert_eq!((px(&d, 0), px(&d, 1)), (0x4444, 0x4444), "kind {kind}");
        assert_ne!(px(&d, 2), 0x4444);
        fill(&mut d, 0x4444);
    }
    // A8888 / X888 key: mask 0xFFFFFF.
    let s: Vec<u8> = [0xAA00_00FFu32, 0x0000_00FE]
        .iter()
        .flat_map(|t| t.to_le_bytes())
        .collect();
    let src = source(&s, 8, 2, 1);
    let b = BlitRecord {
        format: 5,
        key_flag: 1,
        key: 0xFF,
        ..Default::default()
    };
    blt(&mut d, 3, &src, &b, 0, &[Span::row(0, 0, 2)]);
    assert_eq!((px(&d, 0), px(&d, 1)), (0x4444, 0x801F));
    // Row 13 (unreachable): empty mask, key 0 hides everything, other keys draw as row 12.
    let s = [0x80u8, 0x00];
    let src = source(&s, 2, 2, 1);
    fill(&mut d, 0x4444);
    let b = BlitRecord {
        format: 6,
        key_flag: 1,
        key: 0,
        ..Default::default()
    };
    blt(&mut d, 3, &src, &b, 0, &[Span::row(0, 0, 2)]);
    assert_eq!((px(&d, 0), px(&d, 1)), (0x4444, 0x4444));
    let b = BlitRecord { key: 0x80, ..b };
    blt(&mut d, 3, &src, &b, 0, &[Span::row(0, 0, 2)]);
    assert_eq!((px(&d, 0), px(&d, 1)), (0xFFFF, 0x7FFF));
}

#[test]
fn constant_alpha_worked_example_and_source_alpha_unused() {
    let s = src16(&[0x7C00]);
    let src = source(&s, 2, 1, 1);
    let mut d = vec![0u8; 2];
    fill(&mut d, 0x001F);
    let b = BlitRecord {
        format: 2,
        color: 0x8000_0000,
        ..Default::default()
    };
    blt(&mut d, 1, &src, &b, 1, &[Span::row(0, 0, 1)]);
    assert_eq!(px(&d, 0), 0xC010);
    // A4444 texel with alpha 0 still blends; w = 31 at most (never a full replace).
    let s = src16(&[0x0F00]);
    let src = source(&s, 2, 1, 1);
    fill(&mut d, 0);
    let b = BlitRecord {
        format: 0,
        color: 0xFFFF_FFFF,
        ..Default::default()
    };
    blt(&mut d, 1, &src, &b, 1, &[Span::row(0, 0, 1)]);
    assert_eq!(px(&d, 0), per_channel(0x7C00, 0, 31));
}

#[test]
fn modulate_and_modulate_alpha() {
    let s: Vec<u8> = 0x80FF_FFFFu32.to_le_bytes().to_vec();
    let src = source(&s, 4, 1, 1);
    let mut d = vec![0u8; 2];
    let b = BlitRecord {
        format: 4,
        color: 0xFF80_FF00,
        ..Default::default()
    };
    blt(&mut d, 1, &src, &b, 2, &[Span::row(0, 0, 1)]);
    // A' = (128·255+128)>>8 = 128 → A=1; R' = 128 → 16; G' = 254 → 31; B' = 0.
    assert_eq!(px(&d, 0), 0x8000 | (16 << 10) | (31 << 5));
    fill(&mut d, 0x001F);
    blt(&mut d, 1, &src, &b, 3, &[Span::row(0, 0, 1)]);
    let sm = (16 << 10) | (31 << 5);
    assert_eq!(px(&d, 0), per_channel(sm | 0x8000, 0x001F, 128 >> 3));
    // X555: w is the draw colour's alpha.
    let s = src16(&[0x7FFF]);
    let src = source(&s, 2, 1, 1);
    fill(&mut d, 0);
    let b = BlitRecord {
        format: 2,
        color: 0x40FF_FFFF,
        ..Default::default()
    };
    blt(&mut d, 1, &src, &b, 3, &[Span::row(0, 0, 1)]);
    assert_eq!(px(&d, 0), per_channel(0x7FFF, 0, ((255 * 0x40 + 128) >> 8) >> 3));
}

#[test]
fn plain_source_bias_and_texel_widths() {
    // Source 3x2, ox = 1, oy = 1: pointer biased by −(ox·bpp + oy·stride).
    let s = src16(&[1, 2, 3, 4, 5, 6]);
    let mut src = source(&s, 6, 3, 2);
    src.base = -(2 + 6);
    let mut d = vec![0u8; 4 * 3 * 2];
    let b = BlitRecord {
        format: 1,
        ..Default::default()
    };
    blt(&mut d, 4, &src, &b, 0, &[Span::row(1, 1, 3), Span::row(2, 2, 2)]);
    assert_eq!((px(&d, 5), px(&d, 6), px(&d, 7)), (1, 2, 3));
    assert_eq!((px(&d, 10), px(&d, 11)), (5, 6));
    // 8-bit source, one byte per texel.
    let s = [0u8, 0x80, 0x7F];
    let src = source(&s, 3, 3, 1);
    let b = BlitRecord {
        format: 6,
        ..Default::default()
    };
    blt(&mut d, 4, &src, &b, 0, &[Span::row(0, 0, 3)]);
    assert_eq!((px(&d, 0), px(&d, 1), px(&d, 2)), (0x7FFF, 0xFFFF, 0x7FFF));
    // Out-of-range source read is an error.
    let r = render_blt(&[Span::row(0, 0, 4)], &mut surf(&mut d, 4), &src, &b, 0);
    assert_eq!(r, Err(Error::SourceOutOfBounds));
}

#[test]
fn stretch_addressing() {
    // 2x source magnified ×2 with centre sampling (du = 0x8000).
    let s = src16(&[10, 11, 20, 21]);
    let src = source(&s, 4, 2, 2);
    let (u0, v0) = stretch_origin(0x8000, 0x8000, 0, 0);
    assert_eq!((u0, v0), (0x4000, 0x4000));
    let b = BlitRecord {
        u0,
        v0,
        du: 0x8000,
        dv: 0x8000,
        format: 1,
        ..Default::default()
    };
    let mut d = vec![0u8; 4 * 4 * 2];
    let spans: Vec<Span> = (0..4).map(|y| Span::row(0, y, 4)).collect();
    blt(&mut d, 4, &src, &b, 4, &spans);
    let got: Vec<u16> = (0..16).map(|i| px(&d, i)).collect();
    assert_eq!(got, [10, 10, 11, 11, 10, 10, 11, 11, 20, 20, 21, 21, 20, 20, 21, 21]);
    // stretch_origin uses arithmetic shifts and wraps.
    assert_eq!(stretch_origin(-3, 0, 1, 0x10000).0, (-3i32 >> 1).wrapping_sub(1) as u32);
}

#[test]
fn tiled_addressing() {
    // Tile 3x2, origin (ox, oy) = (1, 3): dst (x, y) shows ((x−1) mod 3, (y−3) mod 2).
    let s = src16(&[0, 1, 2, 10, 11, 12]);
    let src = source(&s, 6, 3, 2);
    let (u0, v0) = tile_origin(1, 3, 3, 2).unwrap();
    let b = BlitRecord {
        u0,
        v0,
        format: 1,
        ..Default::default()
    };
    let mut d = vec![0u8; 7 * 2 * 2];
    blt(&mut d, 7, &src, &b, 8, &[Span::row(0, 0, 7), Span::row(2, 1, 5)]);
    for x in 0..7u32 {
        let tx = (x + 2) % 3;
        assert_eq!(px(&d, x as usize), 10 + tx as u16, "row 0 x {x}");
    }
    for x in 2..7u32 {
        assert_eq!(px(&d, 7 + x as usize), ((x + 2) % 3) as u16, "row 1 x {x}");
    }
    assert_eq!(tile_origin(0, 0, 0, 1), None);
    let e = render_blt(&[Span::row(0, 0, 1)], &mut surf(&mut d, 7), &source(&s, 6, 0, 2), &b, 8);
    assert_eq!(e, Err(Error::EmptyTile));
}

#[test]
fn dispatch_range_and_row_from_index() {
    let s = src16(&[0x0005]);
    let src = source(&s, 2, 1, 1);
    let mut d = vec![0u8; 2];
    let b = BlitRecord {
        format: 7,
        ..Default::default()
    };
    let sp = [Span::row(0, 0, 1)];
    assert_eq!(
        render_blt(&sp, &mut surf(&mut d, 1), &src, &b, 0),
        Err(Error::RowOutOfRange { row: 14 })
    );
    let b = BlitRecord {
        format: 1,
        ..Default::default()
    };
    assert_eq!(
        render_blt(&sp, &mut surf(&mut d, 1), &src, &b, 12),
        Err(Error::KindOutOfRange { kind: 12 })
    );
    // key_flag 2 + 2·format 0 = row 2: A1555 key off.
    let b = BlitRecord {
        format: 0,
        key_flag: 2,
        ..Default::default()
    };
    blt(&mut d, 1, &src, &b, 0, &sp);
    assert_eq!(px(&d, 0), 0x0005);
}

#[test]
fn fistp_rounds_half_to_even() {
    let cases = [
        (0.5, 0),
        (1.5, 2),
        (2.5, 2),
        (-0.5, 0),
        (-1.5, -2),
        (-2.6, -3),
        (3.2, 3),
    ];
    for (v, want) in cases {
        assert_eq!(fistp32(v), want, "{v}");
    }
    assert_eq!(fistp32(f64::NAN), i32::MIN);
    assert_eq!(fistp32(3e9), i32::MIN);
}

#[test]
fn caller_helpers() {
    let m: Vec<_> = (0..11).map(src_format_index).collect();
    assert_eq!(
        m,
        [
            None,
            None,
            Some(6),
            None,
            Some(2),
            Some(3),
            Some(1),
            Some(0),
            Some(5),
            Some(4),
            None
        ]
    );
    assert_eq!(caller_key(0, true, 0xFFFF), (1, 0x0FFF));
    assert_eq!(caller_key(3, true, 0x1_FFFF), (1, 0xFFFF));
    assert_eq!(caller_key(4, false, 0xFFFF_FFFF), (0, 0xFF_FFFF));
    assert_eq!(caller_key(6, true, 9), (0, 9));
    assert_eq!(blt_kind(true, true, false, false), 3);
    assert_eq!(blt_kind(false, true, true, true), 10);
    assert_eq!(blt_kind(true, false, false, true), 5);
    assert_eq!(tile_origin(5, 0, 3, 7), Some((1, 0)));
}
