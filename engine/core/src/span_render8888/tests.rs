// SPDX-License-Identifier: GPL-3.0-or-later
//! Tests from the public spec docs/specs/span-render-8888.md only.

use super::blt::modulate;
use super::*;

fn px(v: &[u32]) -> Vec<u8> {
    v.iter().flat_map(|p| p.to_le_bytes()).collect()
}
fn words(b: &[u8]) -> Vec<u32> {
    b.chunks(4)
        .map(|c| u32::from_le_bytes([c[0], c[1], c[2], c[3]]))
        .collect()
}
fn surf(stride: i32) -> Surface {
    Surface { base: 0, stride }
}

// ---- §2 blend rule ----

#[test]
fn blend_worked_example() {
    assert_eq!(blend(0x1234_5678, 0x00FF_0000, 128), 0xFF9A_2B3C);
}

#[test]
fn blend_ends_and_rounding() {
    // w = 0 keeps RGB, forces alpha; w = 256 gives s's RGB; source alpha ignored.
    assert_eq!(blend(0x0012_3456, 0xAB65_4321, 0), 0xFF12_3456);
    assert_eq!(blend(0x0012_3456, 0x0065_4321, 256), 0xFF65_4321);
    // floor((256*0 + 1*1 + 128)/256) = 0; floor((0 + 128*1 + 128)/256) = 1.
    assert_eq!(blend(0, 0x01, 1) & 0xFF, 0);
    assert_eq!(blend(0, 0x01, 128) & 0xFF, 1);
    // s < d: floor((256*255 + 1*(0-255) + 128)/256) = floor(65153/256) = 254.
    assert_eq!(blend(0xFF, 0, 1) & 0xFF, 254);
}

#[test]
fn packed_alpha_gradient_blend_equals_rule_up_to_256() {
    for w in 0..=256u32 {
        for a in (0..256u32).step_by(15) {
            for b in (0..256u32).step_by(17) {
                let d = a << 16 | b << 8 | (255 - a);
                let s = b << 16 | a << 8 | (255 - b);
                assert_eq!(blend_alpha_gradient(d, s, w), blend(d, s, w), "d={d:x} s={s:x} w={w}");
            }
        }
    }
}

#[test]
fn packed_blend_above_256_interferes() {
    // §6 packed formula by hand: d = 0, s = 0x00FFFFFF, w = 512:
    // RB = (0x00FF00FF*512 + 0x00800080) mod 2^32 >> 8 = 0x00FE81FE,
    // G = 0xFF*512 + 0x80 = 0x1FE80; channels wrap (510 -> 0xFE), no clamp.
    assert_eq!(blend_alpha_gradient(0, 0x00FF_FFFF, 512), 0xFFFE_FEFE);
    // w = 65535, d = 1, s = 0: RB = (0xFFFFFFFF*65535 + 0x00800080) mod 2^32 >> 8 = 0x7F00, + 1.
    assert_eq!(blend_alpha_gradient(1, 0, 65535), 0xFF00_0001);
}

// ---- §3, §4 ----

#[test]
fn solid_verbatim_width_zero_and_neighbours() {
    let mut b = px(&[1; 8]);
    let spans = [Span::new(1, 0, 3), Span::new(6, 0, 0)];
    render_solid(&spans, &mut b, surf(32), 0x12AB_CDEF).unwrap();
    assert_eq!(words(&b), [1, 0x12AB_CDEF, 0x12AB_CDEF, 0x12AB_CDEF, 1, 1, 1, 1]);
}

#[test]
fn alpha_weight_and_ends() {
    assert_eq!(render_alpha_weight(0x00FF_FFFF), 0);
    assert_eq!(render_alpha_weight(0x7F00_0000), 127);
    assert_eq!(render_alpha_weight(0x8000_0000), 129);
    assert_eq!(render_alpha_weight(0xFF00_0000), 256);
    let mut b = px(&[0x0012_3456, 0x0012_3456]);
    render_alpha(&[Span::new(0, 0, 1)], &mut b, surf(8), 0x0065_4321, 0xFF00_0000).unwrap();
    render_alpha(&[Span::new(1, 0, 1)], &mut b, surf(8), 0x0065_4321, 0x0011_1111).unwrap();
    assert_eq!(words(&b), [0xFF65_4321, 0xFF12_3456]);
}

// ---- §5, §6 ----

#[test]
fn gradient_worked_example() {
    let g = Gradient {
        start: [0x0010_0000, 0, 0, 0],
        dx: [0x0001_0000, 0, 0, 0],
        dy: [0x0002_0000, 0, 0, 0],
    };
    let mut b = px(&[0; 10]);
    render_gradient(&[Span::new(2, 1, 3)], &mut b, surf(20), &g).unwrap();
    assert_eq!(&words(&b)[7..10], [0xFF00_0014, 0xFF00_0015, 0xFF00_0016]);
    assert_eq!(&words(&b)[..7], [0; 7]);
}

#[test]
fn gradient_r_carry_and_borrow_reach_alpha() {
    let up = Gradient {
        start: [0, 0, 0x00FF_0000, 0],
        dx: [0, 0, 0x0001_0000, 0],
        dy: [0; 4],
    };
    let mut b = px(&[0; 2]);
    render_gradient(&[Span::new(0, 0, 2)], &mut b, surf(8), &up).unwrap();
    assert_eq!(words(&b), [0xFFFF_0000, 0x0000_0000]);
    let down = Gradient {
        start: [0; 4],
        dx: [0, 0, 0xFFFF_0000, 0],
        dy: [0; 4],
    };
    render_gradient(&[Span::new(0, 0, 2)], &mut b, surf(8), &down).unwrap();
    assert_eq!(words(&b), [0xFF00_0000, 0xFEFF_0000]);
}

#[test]
fn gradient_from_dwords_layout() {
    let d: [u32; 12] = core::array::from_fn(|i| i as u32);
    let g = Gradient::from_dwords(&d);
    assert_eq!((g.start, g.dx, g.dy), ([0, 1, 2, 3], [4, 5, 6, 7], [8, 9, 10, 11]));
}

#[test]
fn alpha_gradient_weight_and_blend() {
    assert_eq!(alpha_gradient_weight(0x0100_0000), 256);
    assert_eq!(alpha_gradient_weight(0xFFFF_FFFF), 65535);
    // W = 1.0 copies the ramp colour (R=0x40, G=0x80, B=0xC0); W = 0 keeps d.
    let g = Gradient {
        start: [0x00C0_0000, 0x0080_0000, 0x0040_0000, 0x0100_0000],
        dx: [0, 0, 0, 0xFF00_0000],
        dy: [0; 4],
    };
    let mut b = px(&[0x1122_3344, 0x1122_3344]);
    render_alpha_gradient(&[Span::new(0, 0, 2)], &mut b, surf(8), &g).unwrap();
    assert_eq!(words(&b), [0xFF40_80C0, 0xFF22_3344]);
}

// ---- §8 conversions ----

#[test]
fn widening_per_format() {
    use SourceFormat::*;
    assert_eq!(widen(A4R4G4B4, 0x1F0A), 0x11FF_00AA);
    assert_eq!(widen(A1R5G5B5, 0x8000 | 0x1F << 10 | 0x10 << 5 | 0x01), 0xFFFF_8408);
    assert_eq!(widen(A1R5G5B5, 0x7FFF), 0x00FF_FFFF);
    assert_eq!(widen(X1R5G5B5, 0x0000), 0xFF00_0000);
    assert_eq!(widen(R5G6B5, 0x20 << 5), 0xFF00_8200);
    assert_eq!(widen(R5G6B5, 0xFFFF), 0xFFFF_FFFF);
    assert_eq!(widen(A8R8G8B8, 0x1234_5678), 0x1234_5678);
    assert_eq!(widen(X8R8G8B8, 0x1234_5678), 0xFF34_5678);
    assert_eq!(widen(A8, 0x80), 0x80FF_FFFF);
}

#[test]
fn widening_is_bit_replication_for_every_16_bit_texel() {
    use SourceFormat::*;
    for p in 0..=0xFFFFu32 {
        let c = widen(R5G6B5, p);
        assert_eq!((c >> 19) & 0x1F, p >> 11);
        assert_eq!((c >> 10) & 0x3F, (p >> 5) & 0x3F);
        assert_eq!(c & 0x7, (c >> 5) & 0x7);
        let c = widen(A4R4G4B4, p);
        assert!((0..4).all(|i| (c >> (8 * i)) & 0xF == (c >> (8 * i + 4)) & 0xF));
        assert_eq!(widen(X1R5G5B5, p), widen(A1R5G5B5, p | 0x8000));
    }
}

// ---- §9.2 ops ----

#[test]
fn modulate_examples() {
    assert_eq!(modulate(0xFFFF_FFFF, 0x8080_8080), 0x8080_8080);
    assert_eq!(modulate(0x10, 0xFF) & 0xFF, 16);
    // Never saturates: 255*255 rounds to 254.
    assert_eq!(modulate(0xFFFF_FFFF, 0xFFFF_FFFF), 0xFEFE_FEFE);
}

fn one(k: Kernel, texel: u32, d: u32, desc: BltDesc) -> u32 {
    let mut dst = px(&[d]);
    let src = texel.to_le_bytes();
    let img = SourceImage {
        base: 0,
        stride: 4,
        width: 1,
        height: 1,
    };
    let mut mem = Mem::Split {
        dst: &mut dst,
        src: &src,
    };
    run_kernel(k, &[Span::new(0, 0, 1)], &mut mem, surf(4), img, &desc).unwrap();
    words(&dst)[0]
}

fn kern(format: SourceFormat, keyed: bool, op: Op) -> Kernel {
    Kernel {
        format,
        keyed,
        geometry: Geometry::Direct,
        op,
    }
}

#[test]
fn op_semantics() {
    use SourceFormat::*;
    let d = 0x0112_3456;
    let desc = BltDesc {
        k: 0xFF00_0000,
        ..Default::default()
    };
    // Copy carries C's alpha (format 0/1/4/6) and ignores d.
    assert_eq!(one(kern(A8R8G8B8, false, Op::Copy), 0x00AB_CDEF, d, desc), 0x00AB_CDEF);
    assert_eq!(one(kern(X8R8G8B8, false, Op::Copy), 0x00AB_CDEF, d, desc), 0xFFAB_CDEF);
    // Constant alpha: w = K >> 24 = 255, s = 255 over d = 0 gives 254.
    assert_eq!(
        one(kern(A8R8G8B8, false, Op::ConstantAlpha), 0x00FF_FFFF, 0, desc),
        0xFFFE_FEFE
    );
    // Modulate writes M including alpha.
    let m = BltDesc {
        k: 0x8080_8080,
        ..Default::default()
    };
    assert_eq!(one(kern(A8R8G8B8, false, Op::Modulate), 0xFFFF_FFFF, d, m), 0x8080_8080);
    // Modulate + alpha: B(d, M, M >> 24).
    let mm = modulate(0xFFFF_FFFF, 0x8080_8080);
    assert_eq!(
        one(kern(A8R8G8B8, false, Op::ModulateAlpha), 0xFFFF_FFFF, d, m),
        blend(d, mm, mm >> 24)
    );
    assert_eq!(one(kern(A8, false, Op::ModulateAlpha), 0x00, d, m), 0xFF12_3456);
}

#[test]
fn keyed_compare_raw_texel_with_mask() {
    use SourceFormat::*;
    let d = 0x5555_5555;
    let key = |key| BltDesc {
        key,
        ..Default::default()
    };
    let cases: [(SourceFormat, u32, u32); 6] = [
        (A4R4G4B4, 0xF123, 0x0123),
        (A1R5G5B5, 0x8123, 0x0123),
        (X1R5G5B5, 0xFFFF, 0x7FFF),
        (R5G6B5, 0xF00F, 0xF00F),
        (A8R8G8B8, 0xAB12_3456, 0x0012_3456),
        (X8R8G8B8, 0x0012_3456, 0x0012_3456),
    ];
    for (f, texel, k) in cases {
        assert_eq!(one(kern(f, true, Op::Copy), texel, d, key(k)), d, "{f:?} keyed out");
        assert_ne!(one(kern(f, true, Op::Copy), texel, d, key(k ^ 1)), d, "{f:?} drawn");
        // A key with bits outside the mask never matches.
        assert_ne!(one(kern(f, true, Op::Copy), texel, d, key(k | 0x8000_0000)), d);
        // The unkeyed row ignores the key.
        assert_ne!(one(kern(f, false, Op::Copy), texel, d, key(k)), d);
    }
    // A8: key 0 draws nothing, any other key draws everything.
    assert_eq!(one(kern(A8, true, Op::Copy), 0x40, d, key(0)), d);
    assert_eq!(one(kern(A8, true, Op::Copy), 0x40, d, key(0x40)), 0x40FF_FFFF);
}

// ---- §7, §8 dispatch ----

#[test]
fn table_has_168_distinct_kernels_and_round_trips() {
    let mut seen = std::collections::HashSet::new();
    for row in 0..14 {
        for mode in 0..12 {
            let k = Kernel::from_index(row, mode).unwrap();
            assert_eq!((k.row(), k.mode()), (row, mode));
            seen.insert((k.format as u32, k.keyed, k.geometry as u32, k.op as u32));
        }
    }
    assert_eq!(seen.len(), KERNEL_COUNT);
    let k = Kernel::from_index(13, 11).unwrap();
    assert_eq!(
        (k.format, k.keyed, k.geometry, k.op),
        (SourceFormat::A8, true, Geometry::Tiled, Op::ModulateAlpha)
    );
    let k = Kernel::from_index(3, 5).unwrap();
    assert_eq!(
        (k.format, k.keyed, k.geometry, k.op),
        (SourceFormat::A1R5G5B5, true, Geometry::Stretch, Op::ConstantAlpha)
    );
}

#[test]
fn dispatch_rejects_unchecked_indices() {
    let d = |format, keyed| BltDesc {
        format,
        keyed,
        ..Default::default()
    };
    assert!(Kernel::dispatch(&d(6, 1), 11).is_ok());
    for (f, k, m) in [(7, 0, 0), (0, 2, 0), (0, 0, 12), (0x8000_0000, 0, 0)] {
        assert_eq!(
            Kernel::dispatch(&d(f, k), m),
            Err(SpanError::BadKernelIndex {
                format: f,
                keyed: k,
                mode: m
            })
        );
    }
}

#[test]
fn desc_from_bytes_layout() {
    let mut b = [0u8; 32];
    for (i, v) in b.iter_mut().enumerate() {
        *v = i as u8;
    }
    let d = BltDesc::from_bytes(&b);
    assert_eq!(
        (d.u0, d.format, d.k, d.key, d.keyed),
        (0x0302_0100, 0x1312_1110, 0x1716_1514, 0x1B1A_1918, 0x1C)
    );
}

// ---- §9.1 geometry, strides ----

fn blt(
    k: Kernel,
    spans: &[Span],
    dst: &mut [u8],
    d: Surface,
    src: &[u8],
    img: SourceImage,
    desc: BltDesc,
) -> Result<(), SpanError> {
    run_kernel(k, spans, &mut Mem::Split { dst, src }, d, img, &desc)
}

fn geo(format: SourceFormat, geometry: Geometry) -> Kernel {
    Kernel {
        format,
        keyed: false,
        geometry,
        op: Op::Copy,
    }
}

#[test]
fn direct_with_row_padding_and_negative_stride() {
    // Source 2x2 A8R8G8B8 with a padded 12-byte stride.
    let src = px(&[1, 2, 0xEE, 3, 4, 0xEE]);
    let img = SourceImage {
        base: 0,
        stride: 12,
        width: 0,
        height: 0,
    };
    // Destination with a negative stride: row y lives above row 0.
    let mut dst = px(&[0; 4]);
    let d = Surface { base: 8, stride: -8 };
    blt(
        geo(SourceFormat::A8R8G8B8, Geometry::Direct),
        &[Span::new(0, 0, 2), Span::new(0, 1, 2)],
        &mut dst,
        d,
        &src,
        img,
        BltDesc::default(),
    )
    .unwrap();
    assert_eq!(words(&dst), [3, 4, 1, 2]);
}

#[test]
fn stretch_steps_and_rows() {
    // 4x2 A8 source; du = 0.5 texel, dv = 1 row per 2 dest rows, u0 = 1.0.
    let src = [10u8, 20, 30, 40, 50, 60, 70, 80];
    let img = SourceImage {
        base: 0,
        stride: 4,
        width: 0,
        height: 0,
    };
    let desc = BltDesc {
        u0: 0x1_0000,
        v0: 0,
        du: 0x8000,
        dv: 0x8000,
        ..Default::default()
    };
    let mut dst = px(&[0; 16]);
    blt(
        geo(SourceFormat::A8, Geometry::Stretch),
        &[Span::new(1, 3, 3)],
        &mut dst,
        surf(16),
        &src,
        img,
        desc,
    )
    .unwrap();
    // v = (0x8000*3) >> 16 = 1; u = 0x8000*1 + 0x10000 -> texels 1, 2, 2 of row 1.
    let a: Vec<u32> = words(&dst)[12..16].iter().map(|p| p >> 24).collect();
    assert_eq!(a, [0, 60, 70, 70]);
}

#[test]
fn stretch_u_wraps_in_32_bits() {
    let src = [1u8, 2, 3, 4];
    let img = SourceImage {
        base: 0,
        stride: 4,
        width: 0,
        height: 0,
    };
    // u starts at 0xFFFF0000 (texel 65535, out of the 4-byte source).
    let desc = BltDesc {
        u0: 0xFFFF_0000,
        du: 0x1_0000,
        ..Default::default()
    };
    let mut dst = px(&[0; 2]);
    let r = blt(
        geo(SourceFormat::A8, Geometry::Stretch),
        &[Span::new(0, 0, 2)],
        &mut dst,
        surf(8),
        &src,
        img,
        desc,
    );
    assert_eq!(r, Err(SpanError::SourceOutOfBounds { span: 0 }));
    // u0 = -1 texel, du = 1: texel 65535 then wraps to 0 -- start at x = 1 so u = 0.
    let mut dst = px(&[0; 3]);
    blt(
        geo(SourceFormat::A8, Geometry::Stretch),
        &[Span::new(1, 0, 2)],
        &mut dst,
        surf(12),
        &src,
        img,
        desc,
    )
    .unwrap();
    assert_eq!(words(&dst).iter().map(|p| p >> 24).collect::<Vec<_>>(), [0, 1, 2]);
}

#[test]
fn tiled_start_offset_and_wrap() {
    // 3x2 A8 tile.
    let src = [1u8, 2, 3, 4, 5, 6];
    let img = SourceImage {
        base: 0,
        stride: 3,
        width: 3,
        height: 2,
    };
    let desc = BltDesc {
        u0: 4,
        v0: 1,
        ..Default::default()
    };
    let mut dst = px(&[0; 7]);
    blt(
        geo(SourceFormat::A8, Geometry::Tiled),
        &[Span::new(2, 0, 5)],
        &mut dst,
        surf(28),
        &src,
        img,
        desc,
    )
    .unwrap();
    // tx = (2 + 4) mod 3 = 0, ty = 1: texels 4 5 6 4 5.
    assert_eq!(
        words(&dst)[2..7].iter().map(|p| p >> 24).collect::<Vec<_>>(),
        [4, 5, 6, 4, 5]
    );
    let zero = SourceImage { width: 0, ..img };
    let r = blt(
        geo(SourceFormat::A8, Geometry::Tiled),
        &[Span::new(0, 0, 1)],
        &mut dst,
        surf(28),
        &src,
        zero,
        desc,
    );
    assert_eq!(r, Err(SpanError::ZeroTileSize));
}

// ---- preconditions ----

#[test]
fn errors_leave_memory_untouched() {
    let mut b = px(&[7; 4]);
    assert_eq!(render_solid(&[], &mut b, surf(16), 0), Err(SpanError::EmptySpanList));
    let spans = [Span::new(0, 0, 2), Span::new(3, 0, 2)];
    assert_eq!(
        render_solid(&spans, &mut b, surf(16), 0),
        Err(SpanError::DestOutOfBounds { span: 1 })
    );
    // end before start: huge width.
    let back = [Span { start: 2, end: 1 }];
    assert_eq!(
        render_solid(&back, &mut b, surf(16), 0),
        Err(SpanError::DestOutOfBounds { span: 0 })
    );
    let src = [0u8; 4];
    let img = SourceImage {
        base: 0,
        stride: 4,
        width: 0,
        height: 0,
    };
    let r = blt(
        geo(SourceFormat::A8R8G8B8, Geometry::Direct),
        &[Span::new(0, 0, 1), Span::new(1, 0, 1)],
        &mut b,
        surf(16),
        &src,
        img,
        BltDesc::default(),
    );
    assert_eq!(r, Err(SpanError::SourceOutOfBounds { span: 1 }));
    assert_eq!(words(&b), [7; 4]);
}

// ---- overlapping source and destination ----

fn shared_copy(row: &[u32], src_base: usize, dst_base: usize, w: u16) -> Vec<u32> {
    let mut m = px(row);
    let img = SourceImage {
        base: src_base,
        stride: 0,
        width: 0,
        height: 0,
    };
    let k = geo(SourceFormat::A8R8G8B8, Geometry::Direct);
    let d = Surface {
        base: dst_base,
        stride: 0,
    };
    run_kernel(
        k,
        &[Span::new(0, 0, w)],
        &mut Mem::Shared(&mut m),
        d,
        img,
        &BltDesc::default(),
    )
    .unwrap();
    words(&m)
}

#[test]
fn overlap_reads_live_memory_left_to_right() {
    // Source one pixel left of the destination: each pixel reads the pixel the
    // previous step just wrote, so the first value smears (no memmove).
    assert_eq!(shared_copy(&[1, 2, 3, 4, 5], 0, 4, 4), [1, 1, 1, 1, 1]);
    // Source one pixel right: every texel is read before it is overwritten.
    assert_eq!(shared_copy(&[1, 2, 3, 4, 5], 8, 4, 3), [1, 3, 4, 5, 5]);
    // Same memory, same pixels: unchanged.
    assert_eq!(shared_copy(&[1, 2, 3], 0, 0, 3), [1, 2, 3]);
}

#[test]
fn overlap_across_spans_sees_earlier_spans() {
    // Span 1 copies row 0 to row 1, span 2 copies row 1 to row 2 (source is
    // the row above): span 2 reads what span 1 wrote.
    let mut m = px(&[9, 8, 0, 0, 0, 0]);
    let img = SourceImage {
        base: 0,
        stride: 8,
        width: 0,
        height: 0,
    };
    let d = Surface { base: 8, stride: 8 };
    let k = geo(SourceFormat::A8R8G8B8, Geometry::Direct);
    run_kernel(
        k,
        &[Span::new(0, 0, 2), Span::new(0, 1, 2)],
        &mut Mem::Shared(&mut m),
        d,
        img,
        &BltDesc::default(),
    )
    .unwrap();
    assert_eq!(words(&m), [9, 8, 9, 8, 9, 8]);
}

#[test]
fn overlap_in_place_blend_and_narrow_source() {
    // In place constant alpha on itself: B(d, d, w) keeps RGB, sets alpha.
    let mut m = px(&[0x0012_3456, 0x00AB_CDEF]);
    let img = SourceImage {
        base: 0,
        stride: 8,
        width: 0,
        height: 0,
    };
    let k = Kernel {
        format: SourceFormat::A8R8G8B8,
        keyed: false,
        geometry: Geometry::Direct,
        op: Op::ConstantAlpha,
    };
    let desc = BltDesc {
        k: 0x8000_0000,
        ..Default::default()
    };
    run_kernel(k, &[Span::new(0, 0, 2)], &mut Mem::Shared(&mut m), surf(8), img, &desc).unwrap();
    assert_eq!(words(&m), [0xFF12_3456, 0xFFAB_CDEF]);
    // A 16-bit source overlapping the destination it writes: pixel 0 writes
    // bytes 0..4, which hold texels 0 and 1; pixel 1 reads texel 1 from the
    // freshly written dword (its high half = 0xFFFF of 0xFFFF_FFFF).
    let mut m = px(&[0x0000_7FFF, 0]);
    let img = SourceImage {
        base: 0,
        stride: 8,
        width: 0,
        height: 0,
    };
    let k = geo(SourceFormat::X1R5G5B5, Geometry::Direct);
    run_kernel(
        k,
        &[Span::new(0, 0, 2)],
        &mut Mem::Shared(&mut m),
        surf(8),
        img,
        &BltDesc::default(),
    )
    .unwrap();
    assert_eq!(words(&m), [0xFFFF_FFFF, 0xFFFF_FFFF]);
}

#[test]
fn render_blt_dispatches_by_desc() {
    // format 3 (565), keyed, mode 0: key 0xF800 (pure red) is skipped.
    let src: Vec<u8> = [0xF800u16, 0x07E0].iter().flat_map(|t| t.to_le_bytes()).collect();
    let img = SourceImage {
        base: 0,
        stride: 4,
        width: 0,
        height: 0,
    };
    let desc = BltDesc {
        format: 3,
        keyed: 1,
        key: 0xF800,
        ..Default::default()
    };
    let mut dst = px(&[5, 5]);
    render_blt(
        &[Span::new(0, 0, 2)],
        &mut Mem::Split {
            dst: &mut dst,
            src: &src,
        },
        surf(8),
        img,
        &desc,
        0,
    )
    .unwrap();
    assert_eq!(words(&dst), [5, 0xFF00_FF00]);
}
