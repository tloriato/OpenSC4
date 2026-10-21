// SPDX-License-Identifier: GPL-3.0-or-later
// Copyright (C) 2026 FreeCity4 contributors. See mod.rs for the full notice.

//! Tests from the public statements and examples of docs/specs/span-render-8.md.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

struct Lcg(u64);
impl Lcg {
    fn next(&mut self) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        (self.0 >> 32) as u32
    }
}

fn target(data: &mut [u8], stride: i32) -> Target<'_> {
    Target {
        data,
        origin: 0,
        stride,
    }
}

fn src(origin: usize, stride: i32, width: u32, height: u32) -> SourceImage {
    SourceImage {
        origin,
        stride,
        width,
        height,
    }
}

fn params(format: u32, key_flag: u8, key: u32, colour: u32) -> BltParams {
    BltParams {
        format,
        key_flag,
        key,
        colour,
        ..Default::default()
    }
}

#[test]
fn solid_stores_low_byte() {
    let mut d = [0u8; 16];
    let mut t = target(&mut d, 8);
    render_solid(&[Span::new(1, 1, 5)], &mut t, 0x1234_5678).unwrap();
    assert_eq!(&d[8..16], &[0, 0x78, 0x78, 0x78, 0x78, 0x78, 0, 0]);
    assert!(d[..8].iter().all(|&b| b == 0));
}

#[test]
fn alpha_ignores_argb_and_calls_solid() {
    let mut d = [0u8; 8];
    render_alpha(&[Span::new(0, 0, 8)], &mut target(&mut d, 8), 0xAB, 0x10FF_FFFF).unwrap();
    assert_eq!(d, [0xAB; 8]);
}

#[test]
fn gradient_example_and_channel3_only() {
    let mut g = Gradient::default();
    g.start[3] = 0x00FF_8000;
    g.dx[3] = 0u32.wrapping_sub(0x0001_0000);
    // Channels 0-2 must not matter.
    g.start[0] = 0xDEAD_BEEF;
    g.dx[1] = 0x1234_5678;
    g.dy[2] = 0x7777_7777;
    let mut d = [0u8; 4];
    render_gradient(&[Span::new(0, 0, 4)], &mut target(&mut d, 4), &g).unwrap();
    assert_eq!(d, [0xFF, 0xFE, 0xFD, 0xFC]);
    let mut d2 = [0u8; 4];
    render_alpha_gradient(&[Span::new(0, 0, 4)], &mut target(&mut d2, 4), &g).unwrap();
    assert_eq!(d, d2);
}

#[test]
fn gradient_matches_closed_form_with_wrapping() {
    let mut r = Lcg(7);
    for _ in 0..50 {
        let mut g = Gradient::default();
        for c in 0..4 {
            g.start[c] = r.next();
            g.dx[c] = r.next();
            g.dy[c] = r.next();
        }
        let mut d = vec![0u8; 32 * 4];
        let spans = [Span::new(3, 0, 20), Span::new(0, 2, 32), Span::new(31, 3, 1)];
        render_gradient(&spans, &mut target(&mut d, 32), &g).unwrap();
        for s in spans {
            for x in s.x()..s.x() + s.count() {
                assert_eq!(d[(s.y() * 32 + x) as usize], g.value_at(x, s.y()));
            }
        }
    }
}

#[test]
fn texel_alpha_examples() {
    assert_eq!(texel_alpha(0, 0x8F00), 0x88);
    assert_eq!(texel_alpha(1, 0x7FFF), 0x00);
    assert_eq!(texel_alpha(1, 0x8000), 0xFF);
    assert_eq!(texel_alpha(2, 0), 0xFF);
    assert_eq!(texel_alpha(3, 0), 0xFF);
    assert_eq!(texel_alpha(4, 0x8012_3456), 0x80);
    assert_eq!(texel_alpha(5, 0), 0xFF);
    assert_eq!(texel_alpha(6, 0x5A), 0x5A);
}

#[test]
fn column_examples() {
    assert_eq!(column_value(Column::Modulate, 0xFF, 0xFF), 0xFE);
    assert_eq!(column_value(Column::Modulate, 0xFF, 0x80), 0x80);
    assert_eq!(column_value(Column::Modulate, 0x40, 0xC0), 0x30);
    for a in 0..=255u32 {
        for ca in [0u8, 1, 0x7F, 0x80, 0xFF] {
            assert!(column_value(Column::Modulate, a as u8, ca) <= 254);
            assert_eq!(column_value(Column::ModulateAlpha, a as u8, ca), 0xFF);
            assert_eq!(column_value(Column::ConstantAlpha, a as u8, ca), 0xFF);
            assert_eq!(column_value(Column::Blt, a as u8, ca), a as u8);
        }
    }
}

#[test]
fn table_has_125_bodies_in_owner_range() {
    let bodies: BTreeSet<u32> = DISPATCH.iter().flatten().copied().collect();
    assert_eq!(bodies.len(), 125);
    for b in &bodies {
        assert!((0x0085_E7F0..=0x0086_4A90).contains(b), "{b:#x}");
    }
    // Fills only where §7.2 says the body reads nothing: even rows.
    for (r, row) in DISPATCH.iter().enumerate() {
        for (k, &b) in row.iter().enumerate() {
            if b == BODY_FILL || b == BODY_TILED_FILL {
                assert_eq!(r % 2, 0);
                assert_eq!(b == BODY_TILED_FILL, k >= 8);
            }
        }
    }
}

#[test]
fn blt_kind_composition() {
    assert_eq!(blt_kind(false, false, false, false), 0);
    assert_eq!(blt_kind(true, true, false, false), 3);
    assert_eq!(blt_kind(true, false, true, false), 5);
    assert_eq!(blt_kind(false, true, true, true), 10);
}

/// Renders slot (row, kind) on a fixed random scene; returns the target.
fn run_slot(row: u32, kind: u32, seed: u64, key_from_texel: bool) -> Result<Vec<u8>, SpanError> {
    let mut r = Lcg(seed);
    let slot = Slot::new(row, kind).unwrap();
    let bpp = slot.bpp() as usize;
    let (w, h) = (7u32, 5u32);
    let stride = 64i32;
    let mut sdata = vec![0u8; stride as usize * 64];
    for b in sdata.iter_mut() {
        *b = r.next() as u8;
    }
    let f = slot.format();
    let mut key = r.next() & KEY_MASKS[f as usize];
    if key_from_texel {
        let t = u32::from_le_bytes([
            sdata[2 * bpp],
            sdata[2 * bpp + 1],
            sdata[2 * bpp + 2],
            sdata[2 * bpp + 3],
        ]);
        key = if bpp == 4 { t } else { t & ((1 << (8 * bpp)) - 1) } & KEY_MASKS[f as usize];
    }
    let mut p = params(row / 2, (row & 1) as u8, key, r.next());
    p.u0 = r.next() % 0x4_0000;
    p.v0 = r.next() % 0x4_0000;
    p.du = 0x8000 + r.next() % 0x1_0000;
    p.dv = 0x8000 + r.next() % 0x1_0000;
    let mut d = vec![0x5Cu8; 16 * 8];
    let spans = [Span::new(0, 0, 9), Span::new(2, 1, 12), Span::new(5, 3, 3)];
    let s = src(0, stride, w, h);
    render_blt(&spans, &mut target(&mut d, 16), &sdata, &s, &p, kind)?;
    Ok(d)
}

#[test]
fn shared_bodies_agree_on_every_slot() {
    let mut by_body: BTreeMap<u32, Vec<(u32, u32)>> = BTreeMap::new();
    for r in 0..14 {
        for k in 0..12 {
            by_body
                .entry(DISPATCH[r as usize][k as usize])
                .or_default()
                .push((r, k));
        }
    }
    let mut shared = 0;
    for slots in by_body.values().filter(|v| v.len() > 1) {
        shared += 1;
        for seed in 0..40 {
            let key_hit = seed % 2 == 0;
            let first = run_slot(slots[0].0, slots[0].1, seed, key_hit).unwrap();
            for &(r, k) in &slots[1..] {
                assert_eq!(
                    run_slot(r, k, seed, key_hit).unwrap(),
                    first,
                    "slot ({r},{k}) vs {:?}",
                    slots[0]
                );
            }
        }
    }
    assert!(shared > 0);
}

#[test]
fn every_slot_follows_column_rule() {
    for r in 0..14u32 {
        for k in 0..12u32 {
            for seed in 0..10 {
                let d = run_slot(r, k, seed, false).unwrap();
                let col = Slot::new(r, k).unwrap().column();
                let touched: Vec<u8> = d.iter().copied().filter(|&b| b != 0x5C).collect();
                if matches!(col, Column::ConstantAlpha | Column::ModulateAlpha) {
                    assert!(touched.iter().all(|&b| b == 0xFF), "({r},{k})");
                }
                if col == Column::Modulate {
                    assert!(d.iter().all(|&b| b != 0xFF), "({r},{k})");
                }
            }
        }
    }
}

#[test]
fn plain_blt_each_format() {
    // One row, texels chosen per format; plain addressing src + y*stride + x*bpp.
    let cases: [(u32, Vec<u8>, [u8; 2]); 7] = [
        (0, vec![0x00, 0x8F, 0x00, 0x30], [0x88, 0x33]),
        (1, vec![0xFF, 0x7F, 0x00, 0x80], [0x00, 0xFF]),
        (2, vec![0x12, 0x34, 0x56, 0x78], [0xFF, 0xFF]),
        (3, vec![0x00, 0x00, 0xFF, 0xFF], [0xFF, 0xFF]),
        (4, vec![0x56, 0x34, 0x12, 0x80, 0, 0, 0, 0x01], [0x80, 0x01]),
        (5, vec![0, 0, 0, 0, 0, 0, 0, 0], [0xFF, 0xFF]),
        (6, vec![0x40, 0xC3], [0x40, 0xC3]),
    ];
    for (f, sdata, want) in cases {
        let mut d = [0u8; 2];
        let p = params(f, 0, 0, 0xC000_0000);
        render_blt(
            &[Span::new(0, 0, 2)],
            &mut target(&mut d, 2),
            &sdata,
            &src(0, 8, 2, 1),
            &p,
            0,
        )
        .unwrap();
        assert_eq!(d, want, "f={f}");
        // Modulate with ca = 0xC0.
        render_blt(
            &[Span::new(0, 0, 2)],
            &mut target(&mut d, 2),
            &sdata,
            &src(0, 8, 2, 1),
            &p,
            2,
        )
        .unwrap();
        let m: Vec<u8> = want.iter().map(|&a| ((a as u32 * 0xC0 + 128) >> 8) as u8).collect();
        assert_eq!(d.to_vec(), m, "f={f} modulate");
    }
}

#[test]
fn key_skips_matching_texels_with_full_dword_compare() {
    // A1555 keyed, mask 0x7FFF: texel 0x8123 matches key 0x0123.
    let sdata = [0x23, 0x81, 0x24, 0x81, 0x23, 0x01];
    let mut d = [0x11u8; 3];
    let p = params(1, 1, 0x0123, 0xFF00_0000);
    render_blt(
        &[Span::new(0, 0, 3)],
        &mut target(&mut d, 3),
        &sdata,
        &src(0, 6, 3, 1),
        &p,
        0,
    )
    .unwrap();
    assert_eq!(d, [0x11, 0xFF, 0x11]);
    // An unmasked key (bit 15 set) never matches.
    let mut d = [0x11u8; 3];
    let p = params(1, 1, 0x8123, 0xFF00_0000);
    render_blt(
        &[Span::new(0, 0, 3)],
        &mut target(&mut d, 3),
        &sdata,
        &src(0, 6, 3, 1),
        &p,
        1,
    )
    .unwrap();
    assert_eq!(d, [0xFF, 0xFF, 0xFF]);
    // ModulateAlpha keyed: 0xFF only on non-keyed pixels.
    let mut d = [0x11u8; 3];
    let p = params(1, 1, 0x0123, 0);
    render_blt(
        &[Span::new(0, 0, 3)],
        &mut target(&mut d, 3),
        &sdata,
        &src(0, 6, 3, 1),
        &p,
        3,
    )
    .unwrap();
    assert_eq!(d, [0x11, 0xFF, 0x11]);
}

#[test]
fn row13_compares_no_texel_bits() {
    let sdata = [0x00u8, 0x10, 0x20];
    for (key, want) in [(0u32, [0x77u8; 3]), (5, [0x00, 0x10, 0x20])] {
        let mut d = [0x77u8; 3];
        let p = params(6, 1, key, 0);
        render_blt(
            &[Span::new(0, 0, 3)],
            &mut target(&mut d, 3),
            &sdata,
            &src(0, 3, 3, 1),
            &p,
            0,
        )
        .unwrap();
        assert_eq!(d, want, "key {key}");
    }
    // Row 13 ConstantAlpha reads no texel: tiled divides by W only.
    let mut d = [0u8; 3];
    let p = params(6, 1, 9, 0);
    render_blt(
        &[Span::new(0, 0, 3)],
        &mut target(&mut d, 3),
        &[],
        &src(0, 0, 3, 0),
        &p,
        9,
    )
    .unwrap();
    assert_eq!(d, [0xFF; 3]);
}

#[test]
fn key_flag_byte_is_added_to_row() {
    // f = 0 with key byte 2 selects row 2 (A1555, unkeyed): texel 0x8000 -> 0xFF.
    let sdata = [0x00u8, 0x80];
    let mut d = [0u8; 1];
    let p = params(0, 2, 0, 0);
    assert_eq!(p.row(), 2);
    render_blt(
        &[Span::new(0, 0, 1)],
        &mut target(&mut d, 1),
        &sdata,
        &src(0, 2, 1, 1),
        &p,
        0,
    )
    .unwrap();
    assert_eq!(d, [0xFF]);
    let p = params(7, 0, 0, 0);
    assert_eq!(
        render_blt(
            &[Span::new(0, 0, 1)],
            &mut target(&mut d, 1),
            &sdata,
            &src(0, 2, 1, 1),
            &p,
            0
        ),
        Err(SpanError::BadKernelIndex { row: 14, kind: 0 })
    );
    let p = params(0, 0, 0, 0);
    assert_eq!(
        render_blt(
            &[Span::new(0, 0, 1)],
            &mut target(&mut d, 1),
            &sdata,
            &src(0, 2, 1, 1),
            &p,
            12
        ),
        Err(SpanError::BadKernelIndex { row: 0, kind: 12 })
    );
}

#[test]
fn stretch_uses_logical_shift() {
    // u0 = 0xFFFF0000 -> texel x 0xFFFF (a signed shift would give -1).
    let mut sdata = vec![0u8; 0x1_0000];
    sdata[0xFFFF] = 0x9A;
    let mut p = params(6, 0, 0, 0);
    p.u0 = 0xFFFF_0000;
    let mut d = [0u8; 1];
    render_blt(
        &[Span::new(0, 0, 1)],
        &mut target(&mut d, 1),
        &sdata,
        &src(0, 0, 1, 1),
        &p,
        4,
    )
    .unwrap();
    assert_eq!(d, [0x9A]);
    // Stretch steps from the absolute destination x: (u0 + du*x) >> 16.
    let sdata: Vec<u8> = (0..16).collect();
    let mut p = params(6, 0, 0, 0);
    p.du = 0x8000;
    p.u0 = 0x1_0000;
    let mut d = [0u8; 6];
    render_blt(
        &[Span::new(2, 0, 4)],
        &mut target(&mut d, 6),
        &sdata,
        &src(0, 0, 16, 1),
        &p,
        4,
    )
    .unwrap();
    assert_eq!(d, [0, 0, 2, 2, 3, 3]);
}

#[test]
fn tiled_wraps_and_zero_sizes() {
    // W = 3, H = 2, 8-bit source; start x 4 with u0 1 -> sx (5 mod 3) = 2.
    let sdata = [1u8, 2, 3, 4, 5, 6];
    let mut p = params(6, 0, 0, 0);
    p.u0 = 1;
    p.v0 = 0xFFFF_FFFF; // (0 + v0) mod 2 = 1 in 32-bit unsigned maths.
    let mut d = [0u8; 9];
    render_blt(
        &[Span::new(4, 0, 5)],
        &mut target(&mut d, 9),
        &sdata,
        &src(0, 3, 3, 2),
        &p,
        8,
    )
    .unwrap();
    assert_eq!(&d[4..], &[6, 4, 5, 6, 4]);
    // The tiled fill divides by W: W = 0 errors, H = 0 does not.
    let p = params(6, 0, 0, 0);
    let mut d = [0u8; 2];
    assert_eq!(
        render_blt(
            &[Span::new(0, 0, 2)],
            &mut target(&mut d, 2),
            &[],
            &src(0, 0, 0, 1),
            &p,
            9
        ),
        Err(SpanError::ZeroTileSize)
    );
    render_blt(
        &[Span::new(0, 0, 2)],
        &mut target(&mut d, 2),
        &[],
        &src(0, 0, 1, 0),
        &p,
        9,
    )
    .unwrap();
    assert_eq!(d, [0xFF, 0xFF]);
    // A reading tiled body with H = 0 errors.
    assert_eq!(
        render_blt(
            &[Span::new(0, 0, 2)],
            &mut target(&mut d, 2),
            &sdata,
            &src(0, 0, 1, 0),
            &p,
            8
        ),
        Err(SpanError::ZeroTileSize)
    );
}

#[test]
fn fill_bodies_read_nothing() {
    // X555 unkeyed Blt is the fill body: an empty source is fine.
    let mut d = [0u8; 4];
    let p = params(2, 0, 0, 0);
    render_blt(
        &[Span::new(0, 0, 4)],
        &mut target(&mut d, 4),
        &[],
        &src(0, 0, 0, 0),
        &p,
        0,
    )
    .unwrap();
    assert_eq!(d, [0xFF; 4]);
    // A reading body with no source is rejected before anything is written.
    let mut d = [0u8; 4];
    let p = params(2, 0, 0, 0);
    assert_eq!(
        render_blt(
            &[Span::new(0, 0, 4)],
            &mut target(&mut d, 4),
            &[],
            &src(0, 0, 0, 0),
            &p,
            2
        ),
        Err(SpanError::SourceOutOfBounds { span: 0 })
    );
    assert_eq!(d, [0; 4]);
}

#[test]
fn aliased_source_is_an_ascending_copy() {
    // Source one byte left of the destination: each pixel reads the byte just written.
    let mut d = [1u8, 2, 3, 4, 5];
    let p = params(6, 0, 0, 0);
    let mut t = Target {
        data: &mut d,
        origin: 1,
        stride: 5,
    };
    render_blt_aliased(&[Span::new(0, 0, 4)], &mut t, &src(0, 5, 4, 1), &p, 0).unwrap();
    assert_eq!(d, [1, 1, 1, 1, 1]);
}

#[test]
fn negative_stride_and_bounds() {
    let mut d = [0u8; 8];
    let mut t = Target {
        data: &mut d,
        origin: 4,
        stride: -4,
    };
    render_solid(&[Span::new(0, 1, 4)], &mut t, 0x33).unwrap();
    assert_eq!(d, [0x33, 0x33, 0x33, 0x33, 0, 0, 0, 0]);
    let mut t = target(&mut d, 4);
    assert_eq!(
        render_solid(&[Span::new(0, 0, 1), Span::new(1, 1, 4)], &mut t, 0x44),
        Err(SpanError::DestOutOfBounds { span: 1 })
    );
    assert_eq!(render_solid(&[], &mut t, 0), Err(SpanError::EmptySpanList));
    assert_eq!(d, [0x33, 0x33, 0x33, 0x33, 0, 0, 0, 0]);
}

#[test]
fn overlapping_spans_processed_twice_in_order() {
    // Aliased 8-bit Blt where span 2 rereads pixels span 1 wrote.
    let mut d = [9u8, 0, 0, 0];
    let p = params(6, 0, 0, 0);
    let mut t = Target {
        data: &mut d,
        origin: 1,
        stride: 4,
    };
    render_blt_aliased(
        &[Span::new(0, 0, 2), Span::new(1, 0, 2)],
        &mut t,
        &src(0, 4, 4, 1),
        &p,
        0,
    )
    .unwrap();
    assert_eq!(d, [9, 9, 9, 9]);
}
