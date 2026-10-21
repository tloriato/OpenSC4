// SPDX-License-Identifier: GPL-3.0-or-later
//
// FreeCity4 - an open-source reimplementation of the SimCity 4 engine.
// Copyright (C) 2026 FreeCity4 contributors
// Licensed under GPL-3.0-or-later; see LICENSE at the repository root.

//! Tests from `docs/specs/span-render-565.md` (worked examples, boundaries,
//! rounding, traversal and slot variants).

use super::*;

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }
}

const DST: Dest = Dest { base: 0, stride: 64 };

fn px(mem: &[u8], x: usize, y: usize) -> u16 {
    let a = y * 64 + 2 * x;
    u16::from_le_bytes([mem[a], mem[a + 1]])
}

fn fill(mem: &mut [u8], v: u16) {
    for c in mem.chunks_exact_mut(2) {
        c.copy_from_slice(&v.to_le_bytes());
    }
}

fn lerp(s: u32, d: u32, w: u32) -> u32 {
    (w * s + (32 - w) * d + 16) / 32
}

#[test]
fn blend_worked_example_and_per_channel_lerp() {
    assert_eq!(blend(0xFFFF, 0x0000, 16), 0x8410);
    let mut r = Rng(1);
    for _ in 0..20000 {
        let (s, d, w) = (r.next() as u16, r.next() as u16, r.next() % 33);
        let (s32, d32) = (s as u32, d as u32);
        let want = lerp(s32 >> 11, d32 >> 11, w) << 11
            | lerp((s32 >> 5) & 63, (d32 >> 5) & 63, w) << 5
            | lerp(s32 & 31, d32 & 31, w);
        assert_eq!(blend(s, d, w) as u32, want, "{s:04x} {d:04x} {w}");
    }
    // w = 0 keeps D, w = 32 gives S.
    assert_eq!(blend(0x1234, 0xABCD, 0), 0xABCD);
    assert_eq!(blend(0x1234, 0xABCD, 32), 0x1234);
}

#[test]
fn alpha_weight_thresholds() {
    let first = [
        8, 16, 24, 32, 40, 48, 56, 64, 72, 80, 87, 95, 103, 111, 119, 127, 135, 143, 151, 159, 167, 174, 182, 190, 198,
        206, 214, 222, 230, 238, 246, 254,
    ];
    for (i, &a) in first.iter().enumerate() {
        let w = i as u32 + 1;
        assert_eq!(alpha_weight(a), w);
        assert_eq!(alpha_weight(a - 1), w - 1);
    }
    assert_eq!(alpha_weight(0x80), 16);
    assert_eq!(alpha_weight(255), 32);
}

#[test]
fn solid_low_16_bits_spans_and_zero_length() {
    let mut mem = vec![0u8; 64 * 4];
    let spans = [Span::new(1, 4, 0), Span::new(5, 5, 1), Span::new(0, 2, 2)];
    render_solid(&mut mem, &spans, &DST, 0xDEAD_BEEF).unwrap();
    assert_eq!(
        (px(&mem, 0, 0), px(&mem, 1, 0), px(&mem, 3, 0), px(&mem, 4, 0)),
        (0, 0xBEEF, 0xBEEF, 0)
    );
    assert!((0..32).all(|x| px(&mem, x, 1) == 0));
    assert_eq!((px(&mem, 1, 2), px(&mem, 2, 2)), (0xBEEF, 0));
}

#[test]
fn preconditions_are_rejected() {
    let mut mem = vec![0u8; 64 * 4];
    assert_eq!(render_solid(&mut mem, &[], &DST, 1), Err(SpanError::EmptySpanList));
    let bad = [Span::new(0, 1, 0), Span { start: 5, end: 3 }];
    assert_eq!(
        render_solid(&mut mem, &bad, &DST, 1),
        Err(SpanError::InvalidSpan { index: 1 })
    );
    let two_rows = [Span {
        start: 0,
        end: 1 << 16 | 1,
    }];
    assert_eq!(
        render_solid(&mut mem, &two_rows, &DST, 1),
        Err(SpanError::InvalidSpan { index: 0 })
    );
    let off = [Span::new(31, 33, 3)];
    assert_eq!(
        render_solid(&mut mem, &off, &DST, 7),
        Err(SpanError::OutOfBounds { addr: 3 * 64 + 64 })
    );
    assert_eq!(px(&mem, 31, 3), 7, "pixels before the fault stay written");
}

#[test]
fn negative_stride() {
    let mut mem = vec![0u8; 64 * 4];
    let dst = Dest {
        base: 3 * 64,
        stride: -64,
    };
    render_solid(&mut mem, &[Span::new(2, 3, 1)], &dst, 0x55AA).unwrap();
    assert_eq!(px(&mem, 2, 2), 0x55AA);
}

#[test]
fn render_alpha_examples() {
    let mut mem = vec![0u8; 64 * 2];
    render_alpha(&mut mem, &[Span::new(0, 2, 0)], &DST, 0xABCD_FFFF, 0x80FF_FFFF).unwrap();
    assert_eq!(px(&mem, 0, 0), 0x8410, "src bits 16-31 ignored");
    fill(&mut mem, 0x1234);
    render_alpha(&mut mem, &[Span::new(0, 1, 0)], &DST, 0xFFFF, 0x07FF_FFFF).unwrap();
    assert_eq!(px(&mem, 0, 0), 0x1234, "a8 <= 7 leaves D");
    render_alpha(&mut mem, &[Span::new(0, 1, 0)], &DST, 0xFFFF, 0xFE00_0000).unwrap();
    assert_eq!(px(&mem, 0, 0), 0xFFFF, "a8 >= 254 replaces D");
}

#[test]
fn gradient_b_drift_and_exact_r_g() {
    let mut mem = vec![0u8; 64 * 2];
    let mut g = Gradient::default();
    g.c[0] = 0x0007_FFF8;
    g.dx[0] = 7;
    render_gradient(&mut mem, &[Span::new(0, 3, 0)], &DST, &g).unwrap();
    // Exact value at the third pixel is 0x00080006 (B = 1); the shipped
    // accumulator stays at 0xFFFF (B = 0).
    assert_eq!(g.value(0, 2, 0), 0x0008_0006);
    assert_eq!(px(&mem, 2, 0) & 0x1F, 0);
    // R and G are the top bits of the exact value at every pixel.
    let g2 = Gradient {
        c: [0, 0x0001_2345, 0x0003_0000, 0],
        dx: [0, 0x0000_9137, 0xFFFF_7001, 0],
        dy: [0, 0x0004_0003, 0x0000_1111, 0],
    };
    let mut mem = vec![0u8; 64 * 4];
    render_gradient(&mut mem, &[Span::new(3, 30, 2)], &DST, &g2).unwrap();
    for x in 3..30u32 {
        let p = px(&mem, x as usize, 2) as u32;
        assert_eq!(p >> 11, (g2.value(2, x, 2) >> 19) & 0x1F);
        assert_eq!((p >> 5) & 0x3F, (g2.value(1, x, 2) >> 18) & 0x3F);
    }
    // Producer's white (1.0 -> 0x00FF8000) gives 0xFFFF.
    let white = Gradient {
        c: [0x00FF_8000; 4],
        ..Default::default()
    };
    render_gradient(&mut mem, &[Span::new(0, 4, 0)], &DST, &white).unwrap();
    assert!((0..4).all(|x| px(&mem, x, 0) == 0xFFFF));
}

#[test]
fn gradient_b_step_rounds_toward_minus_infinity() {
    // dx0 = -1: arithmetic shift gives a step of -1 (not 0x1FFFFFFF).
    let mut mem = vec![0u8; 64 * 2];
    let g = Gradient {
        c: [0x0010_0000, 0, 0, 0],
        dx: [0xFFFF_FFFF, 0, 0, 0],
        ..Default::default()
    };
    render_gradient(&mut mem, &[Span::new(0, 2, 0)], &DST, &g).unwrap();
    // start 0x00100000 >> 3 = 0x20000 (B bits 16-20 = 2), then 0x1FFFF (B = 1).
    assert_eq!((px(&mem, 0, 0), px(&mem, 1, 0)), (2, 1));
}

#[test]
fn alpha_gradient_weights() {
    let mut mem = vec![0u8; 64 * 2];
    let full = Gradient {
        c: [0x00FF_8000; 4],
        ..Default::default()
    };
    assert_eq!(full.value(3, 0, 0).wrapping_mul(0x84) >> 26, 32);
    render_alpha_gradient(&mut mem, &[Span::new(0, 2, 0)], &DST, &full).unwrap();
    assert_eq!(px(&mem, 0, 0), 0xFFFF);
    // Out-of-range alpha: w up to 63 and the packed rule applies.
    fill(&mut mem, 0x0000);
    let over = Gradient {
        c: [0x00FF_8000, 0x00FF_8000, 0x00FF_8000, 0x01F0_0000],
        ..Default::default()
    };
    let w = over.value(3, 0, 0).wrapping_mul(0x84) >> 26;
    assert!(w > 32 && w <= 63);
    render_alpha_gradient(&mut mem, &[Span::new(0, 1, 0)], &DST, &over).unwrap();
    assert_eq!(px(&mem, 0, 0), blend(0xFFFF, 0, w));
}

#[test]
fn conversions_examples() {
    assert_eq!(texel_to_565(SourceFormat::A4444, 0x0F8C), 0xFC59);
    assert_eq!(texel_to_565(SourceFormat::X555, 0x0210), 0x0430);
    assert_eq!(texel_to_565(SourceFormat::A1555, 0x8210), 0x0430, "bit 15 ignored");
    assert_eq!(texel_to_565(SourceFormat::X888, 0x00FF_8040), 0xFC08);
    assert_eq!(texel_to_565(SourceFormat::A8888, 0x12FF_8040), 0xFC08);
    assert_eq!(texel_to_565(SourceFormat::R565, 0x1234), 0x1234);
    assert_eq!(texel_to_565(SourceFormat::Index8, 0), 0xFFFF);
    assert_eq!(texel_to_565(SourceFormat::X555, 0x7FFF), 0xFFFF);
    assert_eq!(texel_to_565(SourceFormat::A4444, 0xFFFF), 0xFFFF);
}

/// Source arena at offset 4096 of a shared arena, 256-byte stride.
fn arena() -> (Vec<u8>, Source) {
    (
        vec![0u8; 8192],
        Source {
            base: 4096,
            stride: 256,
            width: 8,
            height: 4,
        },
    )
}

fn put(mem: &mut [u8], src: &Source, f: SourceFormat, x: usize, y: usize, t: u32) {
    let a = src.base as usize + y * src.stride as usize + x * f.bytes();
    mem[a..a + f.bytes()].copy_from_slice(&t.to_le_bytes()[..f.bytes()]);
}

fn blt1(mem: &mut [u8], src: &Source, b: &BltParams, kind: u32) {
    render_blt(mem, &[Span::new(0, 1, 0)], &DST, src, b, kind).unwrap();
}

#[test]
fn column_examples() {
    let (mut mem, src) = arena();
    // ConstantAlpha: weight ca >> 3 is at most 31 (white over black -> R=B=30, G=61).
    put(&mut mem, &src, SourceFormat::R565, 0, 0, 0xFFFF);
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 3,
            argb: 0xFF00_0000,
            ..Default::default()
        },
        1,
    );
    let p = px(&mem, 0, 0);
    assert_eq!((p >> 11, (p >> 5) & 63, p & 31), (30, 61, 30));
    // Modulate: R565 0xFFFF with 0x80808080 -> 0x8410.
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 3,
            argb: 0x8080_8080,
            ..Default::default()
        },
        2,
    );
    assert_eq!(px(&mem, 0, 0), 0x8410);
    // ModulateAlpha with an A8888 texel of alpha 0 leaves D.
    put(&mut mem, &src, SourceFormat::A8888, 0, 0, 0x00FF_FFFF);
    fill(&mut mem[..128], 0x1234);
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 4,
            argb: 0xFFFF_FFFF,
            ..Default::default()
        },
        3,
    );
    assert_eq!(px(&mem, 0, 0), 0x1234);
    // ...and with alpha 255 and a white colour, mA = 255, weight 31.
    put(&mut mem, &src, SourceFormat::A8888, 0, 0, 0xFFFF_FFFF);
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 4,
            argb: 0xFFFF_FFFF,
            ..Default::default()
        },
        3,
    );
    assert_eq!(px(&mem, 0, 0), blend(0xFFFF, 0x1234, 31));
    // 8-bit ModulateAlpha: mA = (texel * ca + 128) >> 8, RGB = colour.
    put(&mut mem, &src, SourceFormat::Index8, 0, 0, 0x80);
    fill(&mut mem[..128], 0);
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 6,
            argb: 0xFFFF_FFFF,
            ..Default::default()
        },
        3,
    );
    assert_eq!(px(&mem, 0, 0), blend(0xFFFF, 0, 16));
    // A4444 Modulate: x * 17 per channel.
    put(&mut mem, &src, SourceFormat::A4444, 0, 0, 0xF8F0);
    blt1(
        &mut mem,
        &src,
        &BltParams {
            format: 0,
            argb: 0xFFFF_FFFF,
            ..Default::default()
        },
        2,
    );
    // m = (c * 255 + 128) >> 8: 0x88 -> 135 (R5 16), 0xFF -> 254 (G6 63).
    assert_eq!(px(&mem, 0, 0), 0x87E0);
}

#[test]
fn colour_key_masks_and_8bit_key_row() {
    let (mut mem, src) = arena();
    // A4444: mask 0x0FFF, the alpha nibble does not take part.
    put(&mut mem, &src, SourceFormat::A4444, 0, 0, 0xF123);
    put(&mut mem, &src, SourceFormat::A4444, 1, 0, 0xF124);
    let b = BltParams {
        format: 0,
        key: 0x123,
        key_flag: 1,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(0, 2, 0)], &DST, &src, &b, 0).unwrap();
    assert_eq!(
        (px(&mem, 0, 0), px(&mem, 1, 0)),
        (0, texel_to_565(SourceFormat::A4444, 0x124))
    );
    // A key with bits outside the mask never matches.
    let b = BltParams { key: 0xF123, ..b };
    render_blt(&mut mem, &[Span::new(0, 1, 0)], &DST, &src, &b, 0).unwrap();
    assert_eq!(px(&mem, 0, 0), texel_to_565(SourceFormat::A4444, 0x123));
    // Key flag clear: no comparison.
    fill(&mut mem[..128], 0);
    let b = BltParams {
        format: 0,
        key: 0x123,
        key_flag: 0,
        ..Default::default()
    };
    blt1(&mut mem, &src, &b, 0);
    assert_ne!(px(&mem, 0, 0), 0);
    // Row 13 (8-bit, keyed) compares no texel bits.
    fill(&mut mem[..128], 0);
    let b = BltParams {
        format: 6,
        key: 0,
        key_flag: 1,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(0, 4, 0)], &DST, &src, &b, 0).unwrap();
    assert!((0..4).all(|x| px(&mem, x, 0) == 0), "key 0 draws nothing");
    let b = BltParams { key: 0x77, ..b };
    render_blt(&mut mem, &[Span::new(0, 4, 0)], &DST, &src, &b, 0).unwrap();
    assert!((0..4).all(|x| px(&mem, x, 0) == 0xFFFF), "key != 0 draws everything");
}

#[test]
fn stretch_uses_logical_shifts_and_absolute_coordinates() {
    // A row of 65536 + 2 R565 texels: u = 0xFFFF0000 maps to texel 0xFFFF,
    // not -1.
    let mut mem = vec![0u8; 128 + 2 * 65538 * 2];
    let src = Source {
        base: 128,
        stride: 2 * 65538,
        width: 65538,
        height: 2,
    };
    let at = |x: usize, y: usize| 128 + y * 2 * 65538 + 2 * x;
    mem[at(0xFFFF, 0)..at(0xFFFF, 0) + 2].copy_from_slice(&0x1111u16.to_le_bytes());
    mem[at(0, 0)..at(0, 0) + 2].copy_from_slice(&0x2222u16.to_le_bytes());
    mem[at(3, 1)..at(3, 1) + 2].copy_from_slice(&0x3333u16.to_le_bytes());
    let b = BltParams {
        u0: 0xFFFF_0000,
        du: 0x1_0000,
        format: 3,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(0, 2, 0)], &DST, &src, &b, 4).unwrap();
    assert_eq!((px(&mem, 0, 0), px(&mem, 1, 0)), (0x1111, 0x2222));
    // Half steps from absolute x = 6: (0x8000 + 0x8000*6) >> 16 = 3; v from y = 1.
    let b = BltParams {
        u0: 0x8000,
        du: 0x8000,
        v0: 0x8000,
        dv: 0x8000,
        format: 3,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(6, 7, 1)], &DST, &src, &b, 4).unwrap();
    // y = 1: (0x8000 + 0x8000) >> 16 = 1.
    assert_eq!(px(&mem, 6, 1), 0x3333);
}

#[test]
fn tiled_wraps_and_origin() {
    let (mut mem, mut src) = arena();
    src.width = 3;
    src.height = 2;
    for x in 0..3 {
        for y in 0..2 {
            put(&mut mem, &src, SourceFormat::R565, x, y, (y * 16 + x + 1) as u32);
        }
    }
    // u0 = 2^32 - 1 (≡ -1): ((x + u0) mod 2^32) mod 3 = (x - 1) mod 3 for x >= 1.
    let b = BltParams {
        u0: u32::MAX,
        v0: 1,
        format: 3,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(1, 8, 0)], &DST, &src, &b, 8).unwrap();
    let row: Vec<u16> = (1..8).map(|x| px(&mem, x, 0)).collect();
    assert_eq!(row, [17, 18, 19, 17, 18, 19, 17]);
    // u0 = 0 at x = 0 but 2^32 mod 3 != 0: x + u0 wraps before the mod.
    let b = BltParams {
        u0: u32::MAX,
        v0: 0,
        format: 3,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(0, 1, 1)], &DST, &src, &b, 8).unwrap();
    // 0xFFFFFFFF mod 3 = 0 (a true -1 mod 3 would be 2): texel (0, 1).
    assert_eq!(px(&mem, 0, 1), 17);
    src.width = 0;
    assert_eq!(
        render_blt(&mut mem, &[Span::new(0, 1, 0)], &DST, &src, &b, 8),
        Err(SpanError::TiledZeroSize)
    );
}

#[test]
fn aliased_source_is_an_ascending_copy() {
    let mut mem = vec![0u8; 64 * 2];
    for x in 0..8 {
        mem[2 * x..2 * x + 2].copy_from_slice(&(x as u16 + 100).to_le_bytes());
    }
    // Source one texel to the left of the destination: the first value smears.
    let src = Source {
        base: -2,
        stride: 64,
        width: 32,
        height: 2,
    };
    let b = BltParams {
        format: 3,
        ..Default::default()
    };
    render_blt(&mut mem, &[Span::new(1, 6, 0)], &DST, &src, &b, 0).unwrap();
    assert!((0..6).all(|x| px(&mem, x, 0) == 100));
    assert_eq!(px(&mem, 6, 0), 106);
}

#[test]
fn dispatch_indices() {
    let (mut mem, src) = arena();
    let spans = [Span::new(0, 1, 0)];
    let b = BltParams {
        format: 7,
        ..Default::default()
    };
    assert_eq!(
        render_blt(&mut mem, &spans, &DST, &src, &b, 0),
        Err(SpanError::SlotOutsideTable { row: 14, kind: 0 })
    );
    let b = BltParams {
        format: 3,
        ..Default::default()
    };
    assert_eq!(
        render_blt(&mut mem, &spans, &DST, &src, &b, 12),
        Err(SpanError::SlotOutsideTable { row: 6, kind: 12 })
    );
    // The key flag byte is added unchecked: f = 0 with flag 2 is row 2.
    put(&mut mem, &src, SourceFormat::A1555, 0, 0, 0x0210);
    let b = BltParams {
        format: 0,
        key_flag: 2,
        ..Default::default()
    };
    render_blt(&mut mem, &spans, &DST, &src, &b, 0).unwrap();
    assert_eq!(px(&mem, 0, 0), 0x0430);
    let s = slot(13, 11).unwrap();
    assert_eq!(
        (s.format, s.keyed, s.op, s.addressing),
        (SourceFormat::Index8, true, BltOp::ModulateAlpha, Addressing::Tiled)
    );
}

#[test]
fn body_table_counts() {
    let mut all: Vec<u32> = SLOT_BODY.iter().flatten().copied().collect();
    assert_eq!(all.len(), 168);
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), 141);
    assert_eq!(all.iter().filter(|&&a| (0x83DE50..0x847CE0).contains(&a)).count(), 139);
    assert_eq!(body_address(6, 0), Some(0x848160));
    assert_eq!(body_address(14, 0), None);
}

/// Wherever two slots run the same body in the original, the
/// parameterised bodies must agree on every input.
#[test]
fn folded_slots_agree() {
    let mut r = Rng(7);
    let mut pairs = Vec::new();
    for a in 0..168u32 {
        for c in a + 1..168 {
            if SLOT_BODY[(a / 12) as usize][(a % 12) as usize] == SLOT_BODY[(c / 12) as usize][(c % 12) as usize] {
                pairs.push((a, c));
            }
        }
    }
    assert_eq!(pairs.len(), 168 - 141);
    for _ in 0..40 {
        let mut base = vec![0u8; 8192];
        base.iter_mut().for_each(|v| *v = r.next() as u8);
        let src = Source {
            base: 4096,
            stride: 256,
            width: 1 + r.next() % 40,
            height: 1 + r.next() % 8,
        };
        let p = BltParams {
            u0: r.next() % 0x10_0000,
            v0: r.next() % 0x4_0000,
            du: r.next() % 0x2_0000,
            dv: r.next() % 0x1_0000,
            argb: r.next(),
            key: r.next() & 0xFF,
            ..Default::default()
        };
        let spans = [Span::new(0, 20, 1), Span::new(5, 31, 3)];
        for &(a, c) in &pairs {
            let run = |s: u32| {
                let mut m = base.clone();
                let row = s / 12;
                let b = BltParams {
                    format: row / 2,
                    key_flag: (row & 1) as u8,
                    ..p
                };
                let res = render_blt(&mut m, &spans, &DST, &src, &b, s % 12);
                (res, m)
            };
            assert!(run(a) == run(c), "slots {a} and {c}");
        }
    }
}

/// Every slot runs on random valid inputs; keyed rows never write a
/// pixel whose masked texel equals the key.
#[test]
fn every_slot_runs_and_keys_hold() {
    let mut r = Rng(11);
    for row in 0..14u32 {
        for kind in 0..12u32 {
            let mut mem = vec![0u8; 8192];
            mem.iter_mut().for_each(|v| *v = (r.next() % 3) as u8);
            let src = Source {
                base: 4096,
                stride: 256,
                width: 8,
                height: 4,
            };
            let b = BltParams {
                format: row / 2,
                key_flag: (row & 1) as u8,
                argb: r.next(),
                ..Default::default()
            };
            let before = mem.clone();
            render_blt(&mut mem, &[Span::new(0, 8, 0)], &DST, &src, &b, kind).unwrap();
            let s = slot(row, kind).unwrap();
            if s.keyed && s.addressing == Addressing::Plain && s.format != SourceFormat::Index8 {
                for x in 0..8 {
                    let n = s.format.bytes();
                    let t = (0..n).rev().fold(0u32, |v, i| v << 8 | before[4096 + x * n + i] as u32);
                    if t & s.format.key_mask() == 0 {
                        assert_eq!(px(&mem, x, 0), px(&before, x, 0), "row {row} kind {kind} x {x}");
                    }
                }
            }
        }
    }
}
