// SPDX-License-Identifier: GPL-3.0-or-later
//! Literal expectations transcribed from render3d §§8,10,11,12 BEFORE implementation.
//! These contain no renderer-derived oracle or algorithm.

pub type Rect = [i32; 4];
pub struct DirtyFixture {
    pub name: &'static str,
    pub queued: &'static [(u32, Rect)],
    pub incoming: (u32, Rect),
    pub expected: &'static [(u32, Rect)],
    pub area: i32,
}
pub const DIRTY: &[DirtyFixture] = &[
    DirtyFixture {
        name: "D1 overlap",
        queued: &[(1, [0, 0, 100, 100])],
        incoming: (1, [50, 0, 150, 100]),
        expected: &[(6, [0, 0, 150, 100])],
        area: 15000,
    },
    DirtyFixture {
        name: "D2 separate",
        queued: &[(1, [0, 0, 10, 10])],
        incoming: (1, [500, 500, 510, 510]),
        expected: &[(1, [0, 0, 10, 10]), (1, [500, 500, 510, 510])],
        area: 200,
    },
    DirtyFixture {
        name: "D3 mode zero",
        queued: &[(0, [0, 0, 100, 100])],
        incoming: (1, [0, 0, 100, 100]),
        expected: &[(6, [0, 0, 100, 100])],
        area: 10000,
    },
    DirtyFixture {
        name: "D4 recursive overcount, first tie wins",
        queued: &[(1, [0, 0, 50, 100]), (1, [100, 0, 200, 100])],
        incoming: (1, [40, 0, 110, 100]),
        expected: &[(6, [0, 0, 200, 100])],
        area: 31000,
    },
];
// D5: 15 separate 10x10 nodes at x = 0, 1000, ..., 14000; incoming
// at x=15000 appends (16 nodes); x=16000 MUST merge despite score < -2500.
// Spacing corrected from 100: that gap scores -900, which would merge early.
pub const D5_NODE_BOUNDARY: (usize, usize) = (15, 16);
pub const FILTER: &[(Rect, bool, Rect)] = &[
    ([-10, 590, 20, 700], true, [0, 590, 20, 600]), // D6
    ([800, 0, 900, 10], false, [800, 0, 900, 10]),
    ([5, 0, 3, 10], true, [5, 0, 3, 10]), // no positive-area test
    ([-20, 0, 0, 10], false, [-20, 0, 0, 10]),
    ([0, 12, 10, 9], true, [0, 12, 10, 9]),
];

// B1/B2/B3/B4: arguments are srcX, srcY, width, height, dstX, dstY.
pub const B1: [i32; 6] = [0, 0, 800, 600, 0, 0];
pub const B2: [i32; 4] = [10, 10, 50, 50]; // partial gate false => false, no calls
pub const B3: [i32; 6] = [0, 2, 797, 598, 3, 0]; // shake (3,-2)
pub const B4: [i32; 6] = [0, 0, 5, 600, 7, 0]; // Save(2,0,10,600), shake (5,0)
pub const PROBE_ORDER: &[(u8, u32)] = &[(6, 10), (7, 10), (6, 11), (7, 11)];

// Queue cases: kind 1 retains OLD, kind 2 has OLD and CURRENT, kinds
// 0/3/4/5 use CURRENT when processed. Move mode is (flags & 0x0200) != 0.
pub const OLD: ([f32; 3], [f32; 3]) = ([0.0, 0.0, 0.0], [1.0, 1.0, 1.0]);
pub const CURRENT: ([f32; 3], [f32; 3]) = ([2.0, 0.0, 0.0], [3.0, 1.0, 1.0]);
pub const EMPTY: ([f32; 3], [f32; 3]) = ([1.0; 3], [0.0; 3]);
// P1 plus duplicate and high-bit discriminators: unsigned keys, stable equals.
pub const LIST_ADDS: &[(u64, u32)] = &[(1, 5), (2, 2), (3, 5), (1, 5), (4, 0x80000000)];
pub const LIST_ORDER: &[u64] = &[2, 1, 3, 1, 4];

// S1 prepend + monotonically increasing handles.
pub const S1_HANDLES: [u32; 2] = [0, 1];
pub const S1_ORDER: [u32; 2] = [1, 0];
// S2 overload curves at t=1,duration=2; equality at t=2 keeps the node.
pub const S2: (f32, f32) = (2.5, 60.0);
// S3 weight at radius and beyond; only beyond skips phase advance.
pub const S3: [(f32, bool); 2] = [(0.0, true), (-3.0, false)];
// S4 zoom -> repeated extended product; rounded only when consumed.
pub const S4: &[(i32, f64)] = &[
    (3, 0.6000000238418579),
    (4, 1.0),
    (5, 1.0),
    (1, 0.2160000257492075),
];
// S5 ties round to even. S6: disabled => offsets zero, no node/phase/time mutation.
pub const S5: &[(f64, i32)] = &[(2.5, 2), (3.5, 4), (-2.5, -2)];
pub const S6_OFFSETS: (i32, i32) = (0, 0);
